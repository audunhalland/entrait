#![feature(generic_associated_types)]
#![feature(type_alias_impl_trait)]

use core::future::Future;
use entrait::*;

async fn foo(deps: impl Bar) {
    deps.bar().await;
}

trait Foo: Copy {
    type Fut: Future<Output = ()>;

    fn foo(self) -> Self::Fut;
}

impl<T: 'static> Foo for &Impl<T>
where
    Self: Bar,
{
    type Fut = impl Future<Output = ()>;

    fn foo(self) -> Self::Fut {
        async move { foo(self).await }
    }
}

async fn bar<D>(_: D) {}

trait Bar: Copy {
    type Fut: Future<Output = ()>;

    fn bar(self) -> Self::Fut;
}

impl<T: 'static> Bar for &Impl<T> {
    type Fut = impl Future<Output = ()>;

    fn bar(self) -> Self::Fut {
        async move { bar(self).await }
    }
}

fn assert_send<T: Send>(_: T) {}
fn assert_sync<T: Sync>(_: T) {}

#[tokio::test]
async fn test() {
    let app = Impl::new(());

    assert_send(app.foo());
    assert_sync(app.foo());

    app.foo().await;
}

// Delegation:
trait FoobarStatic: Copy {
    type Fut: Future<Output = ()>;

    fn foobar(self) -> Self::Fut;
}

trait DelegateFoobar<'s, T> {
    type Target: FoobarStatic + 's;

    fn wrap(_impl: &'s Impl<T>) -> Self::Target;
}

impl<'s, T: 'static> FoobarStatic for &'s Impl<T>
where
    T: DelegateFoobar<'s, T>,
{
    type Fut = impl Future<Output = ()>;

    fn foobar(self) -> Self::Fut {
        async move { T::wrap(self).foobar().await }
    }
}

fn takes_foobar_static(deps: impl FoobarStatic) {}
