use cc_launch_lib::cursor::fake_ip::FakeIpMap;

#[test]
fn allocator_maps_and_releases_cursor_hosts() {
    let mut map = FakeIpMap::allocate(&["api2.cursor.sh".into(), "api3.cursor.sh".into()]).unwrap();
    assert_eq!(
        map.address("API2.CURSOR.SH"),
        Some("127.0.0.2".parse().unwrap())
    );
    assert_eq!(
        map.hostname("127.0.0.3".parse().unwrap()),
        Some("api3.cursor.sh")
    );
    map.release("api2.cursor.sh");
    assert_eq!(map.address("api2.cursor.sh"), None);
}

#[test]
fn allocator_rejects_non_loopback_or_unknown_addresses() {
    let map = FakeIpMap::allocate(&["api2.cursor.sh".into()]).unwrap();
    assert!(map.hostname("10.0.0.1".parse().unwrap()).is_none());
    assert!(map.hostname("127.0.0.254".parse().unwrap()).is_none());
}
