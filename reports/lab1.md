## 总结
实现sys_task_info。

对于TaskStatus，直接硬编码了Running的值；扩展了PCB中的字段以容纳task_syscall_times和task_time。

在trap处更新task_syscall_times，在run_first_task和每次run_next_task时更新task_time。

但是遇到了一个非常扯淡的问题，浪费了我很长的时间，详见timer.rs。也许在评测机上不会出现这种问题，但我还是把这个workaround保留下来了。

```
...
Panicked at src/bin/ch3_sleep.rs:16, assertion failed: current_time > 0
current time_msec = 0
...
```

更新：我发现我必须撤销我对timer.rs的改动才能通过评测，但就会导致我自己的机器不能通过测试。怎么会这样？

## 问答
1. 
```
[kernel] PageFault in application, bad addr = 0x0, bad instruction = 0x804003c4, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
[kernel] IllegalInstruction in application, kernel killed it.
```
RustSBI版本：Prereleased 2023-10-27

2. 
    1. a0代表TrapContext；切换特权级时恢复上下文，及执行第一个任务时从S态切换到U态。
    2. sstatus，spec，sscratch；sstatus保存特权级状态，spec保存之前的pc，sscratch保存之前的sp。
    3. x2代表sp，已经保存在sscratch，x4未使用。
    4. sp变为用户栈指针，sscratch变为内核栈指针。
    5. sret；将sstatus恢复为之前的特权级状态，将pc恢复为spec的值。
    6. sp变为内核栈指针，sscratch变为用户栈指针。
    7. ecall。


## 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

（无）

2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

https://learningos.cn/rCore-Tutorial-Guide-2023A/chapter3/

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。