# Lab1 开发日志

## 2026-04-02

### 阶段 1：Alarm Clock（非忙等睡眠）
- 实现 `thread::sleep(ticks)`：`ticks<=0` 立即返回；否则将当前线程按唤醒时刻插入全局睡眠队列后阻塞。
- 新增全局睡眠队列 `SLEEP_LIST`（按 `wake_tick` 升序）。
- 新增 `thread::wake_sleeping_threads()`，在时钟中断里批量唤醒到期线程。
- 在 `trap` 的 `SupervisorTimer` 分支中接入唤醒逻辑。

### 验证
- 待执行 `alarm-*` 测试。

### Commit
- 待提交。

### 验证
- `cargo check --features test-alarm-zero` 通过。
- 运行 `make test-alarm-zero` 时因环境缺少 `qemu-system-riscv64` 无法执行集成测试。

### Commit
- `pending`
