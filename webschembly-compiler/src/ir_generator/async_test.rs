mod bar {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    pub struct MyRunner {
        pub count: u32,
    }

    impl MyRunner {
        pub async fn recursive_work(&mut self, n: u32) {
            self.count += 1;
            if n == 0 {
                return;
            }

            yield_once(self).await;

            Box::pin(self.recursive_work(n - 1)).await;
        }
    }

    // 一度だけ Pending を返す Future
    fn yield_once(s: &mut MyRunner) -> YieldOnce {
        YieldOnce(false)
    }

    struct YieldOnce(bool);
    impl Future for YieldOnce {
        type Output = ();
        fn poll(mut self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            if self.0 {
                Poll::Ready(())
            } else {
                self.0 = true;
                Poll::Pending
            }
        }
    }

    /// スタックを消費しないエグゼキュータ
    pub fn block_on_trampoline<F: Future>(future: F) -> F::Output {
        let mut main_future = Box::pin(future);
        let waker = Waker::noop();
        let mut cx = Context::from_waker(&waker);

        loop {
            match main_future.as_mut().poll(&mut cx) {
                Poll::Ready(res) => return res,
                Poll::Pending => {
                    continue;
                }
            }
        }
    }

    #[test]
    fn test_safe_async_trampoline() {
        let mut runner = MyRunner { count: 0 };

        block_on_trampoline(runner.recursive_work(100_000));

        assert_eq!(runner.count, 100_001);
    }
}

mod foo {
    use std::future::Future;
    use std::pin::Pin;
    use std::task::{Context, Poll, Waker};

    // 1. 次に何をすべきかを示す列挙型
    enum Step<'a> {
        Next(Pin<Box<dyn Future<Output = Step<'a>> + 'a>>),
        Done,
    }

    pub struct MyRunner {
        pub count: u32,
    }

    impl MyRunner {
        // async関数ではなく、Stepを返す「一歩」の処理として定義
        fn step(&mut self, n: u32) -> Pin<Box<dyn Future<Output = Step<'_>> + '_>> {
            Box::pin(async move {
                self.count += 1;
                if n == 0 {
                    return Step::Done;
                }

                Step::Next(self.step(n - 1))
            })
        }
    }

    pub fn block_on_trampoline(runner: &mut MyRunner, n: u32) {
        let waker = Waker::noop();
        let mut cx = Context::from_waker(&waker);

        // Option で管理することで、所有権を一時的に取り出せるようにする
        let mut current_step = Some(Step::Next(runner.step(n)));

        loop {
            match current_step.take() {
                Some(Step::Next(mut fut)) => {
                    match fut.as_mut().poll(&mut cx) {
                        Poll::Ready(next_step) => {
                            // 次のステップ（新しいFuture）に更新
                            current_step = Some(next_step);
                        }
                        Poll::Pending => {
                            // まだ終わっていないので、今取り出した fut を元に戻してループを継続
                            current_step = Some(Step::Next(fut));
                            continue;
                        }
                    }
                }
                Some(Step::Done) | None => break,
            }
        }
    }

    #[test]
    fn test_true_safe_trampoline() {
        let mut runner = MyRunner { count: 0 };

        // 100万回でもスタックオーバーフローしない
        block_on_trampoline(&mut runner, 1_000_000);

        assert_eq!(runner.count, 1_000_001);
    }
}
