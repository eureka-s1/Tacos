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
- `8d4899b` (`lab1: implement alarm clock sleep queue`)

### 验证
- `cargo check --features test-alarm-zero` 通过。
- 运行 `make test-alarm-zero` 时因环境缺少 `qemu-system-riscv64` 无法执行集成测试。

### Commit
- `8d4899b` (`lab1: implement alarm clock sleep queue`)

### 阶段 2：Priority Scheduling（基础）
- 新增 `Priority` 调度器（高优先级优先，同优先级 FIFO/RR）。
- `thread::set_priority/get_priority` 实现完成，支持范围截断。
- 新线程 `spawn` 时若优先级更高，当前线程立即让出 CPU。
- `Semaphore` 等待队列改为按优先级分桶，`up` 唤醒最高优先级 waiter，并在可抢占场景触发调度。
- `Condvar` waiter 改为按优先级分桶，`notify_one/notify_all` 按优先级唤醒。

### 验证
- `cargo check --features test-priority-preempt` 通过。
- `cargo check --features test-priority-change` 通过。
- `cargo check --features test-priority-fifo` 通过。
- `cargo check --features test-priority-sema` 通过。
- `cargo check --features test-priority-condvar` 通过。
- 集成运行测试受限：本环境缺少 `qemu-system-riscv64`。

### Commit
- `pending`
