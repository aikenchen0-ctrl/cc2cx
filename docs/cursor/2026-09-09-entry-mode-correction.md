# 入口模式互斥修正

日期：2026-09-09

## 结论

cc2cx 有两套 Cursor 接入，来自两个参考项目，**不能叠在同一运行实例里验收**。

| 模式 | 学自 | 作用 |
|---|---|---|
| P 显式代理 | cursor-byok | `http.proxy` + `disableHttp2` + hudsucker，不改 Hosts |
| T 透明入口 | cursor-fake 的 fake-IP 形状 | Hosts → `127.0.0.x:443` 终止 TLS，不设 `http.proxy` |

cursor-fake 不解密。go-mitmproxy 不做透明代理。把 fake-IP 接到现有 MITM/backend 是对的；把 Hosts 和 `http.proxy` 一起打开是错的。

## 逻辑谬误

1. 「两套一起开覆盖更全」——Agent `Run` 会被 `--proxy-server` 吸进 MITM，透明 `:443` 没有 Agent 证据。
2. 「Hosts 映射官方名，MITM 再拨官方名」——解析结果是 fake-IP，旁路会环回。
3. 「隔离窗口 reconnecting/HTTP 408 说明透明入口坏了」——该窗口同时有 Hosts、MITM、`--proxy-server`，408 发生在模式 P。
4. 设计里 fake-IP「保存上游真实 IP」——当前实现没有、也不该有；模式 T 只本地终止，不透传。

## 遗漏

- E2E 未声明模式。
- `cursor_e2e_lab` 用 `launch_with_proxy` 却又 `enable_transparent_entry(system hosts)`。
- 文档把「停透明入口再停 MITM 再恢复 settings」写成一条流水线。

## 已改文档

- `docs/superpowers/specs/2026-09-08-cursor-transparent-entry-design.md`
- `docs/cursor/baseline.md` 第 8 节
- `docs/cursor/p2-contract.md` 边界段
- `src-tauri/examples/cursor_e2e_lab.rs` 文件头

Harness `start()` 与 `cursor_e2e_lab` 已改为一次只开一种入口。任何真实窗口结果仍须带模式标记：模式 T 无 `--proxy-server` / `http.proxy`，模式 P 无 Hosts。
