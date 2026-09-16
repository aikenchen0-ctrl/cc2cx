//! Windows Root certificate-store probe for the Cursor-local CA.
//!
//! The comparison is against the complete DER encoding. Matching by common name alone could
//! report an unrelated certificate as trusted and would make uninstall/status unsafe.

use std::{ffi::c_void, io, ptr, slice};

use sha1::{Digest, Sha1};

use windows_sys::Win32::Security::Cryptography::{
    CertAddEncodedCertificateToStore, CertCloseStore, CertDeleteCertificateFromStore,
    CertEnumCertificatesInStore, CertFindCertificateInStore, CertFreeCertificateContext,
    CertOpenStore, CERT_FIND_SHA1_HASH, CERT_STORE_ADD_REPLACE_EXISTING,
    CERT_STORE_MAXIMUM_ALLOWED_FLAG, CERT_STORE_OPEN_EXISTING_FLAG, CERT_STORE_PROV_SYSTEM_W,
    CERT_STORE_READONLY_FLAG, CERT_SYSTEM_STORE_CURRENT_USER, CRYPT_INTEGER_BLOB,
    X509_ASN_ENCODING,
};

use super::super::error::{CursorError, Result};

const ROOT_STORE: [u16; 5] = [b'R' as u16, b'O' as u16, b'O' as u16, b'T' as u16, 0];
const INTERMEDIATE_STORE: [u16; 3] = [b'C' as u16, b'A' as u16, 0];

pub(super) fn is_installed(cert: &str) -> Result<bool> {
    let der = pem::parse(cert)
        .map_err(|error| CursorError::Config(format!("解析 CA PEM 失败: {error}")))?
        .into_contents();
    // The product only manages the current-user Root store. Do not inspect the machine-wide
    // store: it can be policy-controlled or prompt for elevated access, and a machine-level
    // certificate is outside this integration's ownership boundary.
    let store = open_store(&ROOT_STORE, CERT_SYSTEM_STORE_CURRENT_USER, true)?;
    let found = enumerate_matches(store, &der);
    let close_result = unsafe { CertCloseStore(store, 0) };
    if close_result == 0 {
        return Err(CursorError::Config(format!(
            "关闭 Windows 证书库失败: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(found)
}

pub(super) fn install(cert: &str) -> Result<()> {
    let der = pem::parse(cert)
        .map_err(|error| CursorError::Config(format!("解析 CA PEM 失败: {error}")))?
        .into_contents();
    let der_len = der
        .len()
        .try_into()
        .map_err(|_| CursorError::Config("CA 证书长度超过 Windows API 限制".to_string()))?;
    // The Windows certificate wizard can place an imported CA in the user's intermediate
    // store when the destination is not explicitly selected. Move only this exact managed DER
    // out of CA before publishing it to Root, so a prior mistaken import cannot keep the UI in
    // an "installed but untrusted" loop.
    remove_exact_from_store(&INTERMEDIATE_STORE, &der)?;
    let store = open_store(&ROOT_STORE, CERT_SYSTEM_STORE_CURRENT_USER, false)?;
    let added = unsafe {
        CertAddEncodedCertificateToStore(
            store,
            X509_ASN_ENCODING,
            der.as_ptr(),
            der_len,
            CERT_STORE_ADD_REPLACE_EXISTING,
            ptr::null_mut(),
        )
    };
    let close_result = unsafe { CertCloseStore(store, 0) };
    if added == 0 {
        return Err(CursorError::Config(format!(
            "写入 Windows Root 证书库失败: {}",
            io::Error::last_os_error()
        )));
    }
    if close_result == 0 {
        return Err(CursorError::Config(format!(
            "关闭 Windows 证书库失败: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(())
}

pub(super) fn uninstall(cert: &str) -> Result<()> {
    let der = pem::parse(cert)
        .map_err(|error| CursorError::Config(format!("解析 CA PEM 失败: {error}")))?
        .into_contents();
    let store = open_store(&ROOT_STORE, CERT_SYSTEM_STORE_CURRENT_USER, false)?;
    let mut previous = ptr::null();
    loop {
        let current = unsafe { CertEnumCertificatesInStore(store, previous) };
        if current.is_null() {
            break;
        }
        let encoded = unsafe {
            slice::from_raw_parts((*current).pbCertEncoded, (*current).cbCertEncoded as usize)
        };
        if encoded == der {
            let result = unsafe { CertDeleteCertificateFromStore(current) };
            if result == 0 {
                let _ = unsafe { CertCloseStore(store, 0) };
                return Err(CursorError::Config(format!(
                    "从 Windows Root 证书库删除失败: {}",
                    io::Error::last_os_error()
                )));
            }
            break;
        }
        previous = current;
    }
    let close_result = unsafe { CertCloseStore(store, 0) };
    if close_result == 0 {
        return Err(CursorError::Config(format!(
            "关闭 Windows 证书库失败: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(())
}

fn enumerate_matches(
    store: windows_sys::Win32::Security::Cryptography::HCERTSTORE,
    der: &[u8],
) -> bool {
    // CertEnumCertificatesInStore is useful for diagnostics, but CertFindCertificateInStore is
    // the stable lookup path for logical Windows stores. The latter also avoids depending on
    // context-enumeration behavior that can differ between certmgr and CryptoAPI callers.
    let digest = Sha1::digest(der);
    let mut hash = CRYPT_INTEGER_BLOB {
        cbData: digest.len() as u32,
        pbData: digest.as_ptr() as *mut u8,
    };
    let found = unsafe {
        CertFindCertificateInStore(
            store,
            X509_ASN_ENCODING,
            0,
            CERT_FIND_SHA1_HASH,
            (&mut hash as *mut CRYPT_INTEGER_BLOB).cast::<c_void>(),
            ptr::null(),
        )
    };
    if !found.is_null() {
        let encoded = unsafe {
            slice::from_raw_parts((*found).pbCertEncoded, (*found).cbCertEncoded as usize)
        };
        let matches = encoded == der;
        unsafe { CertFreeCertificateContext(found) };
        return matches;
    }

    // Keep a defensive fallback for stores/providers that do not expose SHA-1 indexing. This
    // still compares the complete DER and never trusts a subject/common-name-only match.
    let mut previous = ptr::null();
    loop {
        let current = unsafe { CertEnumCertificatesInStore(store, previous) };
        if current.is_null() {
            return false;
        }

        let encoded = unsafe {
            slice::from_raw_parts((*current).pbCertEncoded, (*current).cbCertEncoded as usize)
        };
        let matches = encoded == der;
        if matches {
            unsafe { CertFreeCertificateContext(current) };
            return true;
        }
        previous = current;
    }
}

fn open_store(
    store_name: &[u16],
    scope: u32,
    readonly: bool,
) -> Result<windows_sys::Win32::Security::Cryptography::HCERTSTORE> {
    let flags = scope
        | CERT_STORE_OPEN_EXISTING_FLAG
        | if readonly {
            CERT_STORE_READONLY_FLAG
        } else {
            CERT_STORE_MAXIMUM_ALLOWED_FLAG
        };
    let store = unsafe {
        CertOpenStore(
            CERT_STORE_PROV_SYSTEM_W,
            0,
            0,
            flags,
            store_name.as_ptr().cast(),
        )
    };
    if store.is_null() {
        return Err(CursorError::Config(format!(
            "打开 Windows Root 证书库失败: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(store)
}

fn remove_exact_from_store(store_name: &[u16], der: &[u8]) -> Result<()> {
    let store = open_store(store_name, CERT_SYSTEM_STORE_CURRENT_USER, false)?;
    let mut previous = ptr::null();
    loop {
        let current = unsafe { CertEnumCertificatesInStore(store, previous) };
        if current.is_null() {
            break;
        }
        let encoded = unsafe {
            slice::from_raw_parts((*current).pbCertEncoded, (*current).cbCertEncoded as usize)
        };
        if encoded == der {
            let deleted = unsafe { CertDeleteCertificateFromStore(current) };
            if deleted == 0 {
                let _ = unsafe { CertCloseStore(store, 0) };
                return Err(CursorError::Config(format!(
                    "从 Windows 中间证书库迁移 CC2CX CA 失败: {}",
                    io::Error::last_os_error()
                )));
            }
            break;
        }
        previous = current;
    }
    let closed = unsafe { CertCloseStore(store, 0) };
    if closed == 0 {
        return Err(CursorError::Config(format!(
            "关闭 Windows 中间证书库失败: {}",
            io::Error::last_os_error()
        )));
    }
    Ok(())
}
