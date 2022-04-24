#![feature(generic_associated_types)]

use entrait::entrait;
use unimock::*;

#[entrait(Bar, async_trait = true, unimock = true)]
async fn bar<A>(_: &A) -> i32 {
    unimplemented!()
}

#[entrait(Baz, async_trait = true, unimock = true)]
async fn baz<A>(_: &A) -> i32 {
    unimplemented!()
}

mod inline_bounds {
    use super::*;
    use entrait::entrait;

    #[entrait(Sum, async_trait = true)]
    async fn sum<A: Bar + Baz>(a: &A) -> i32 {
        a.bar().await + a.baz().await
    }

    #[tokio::test]
    async fn test_mock() {
        let mock = mock([
            bar::Fn::each_call(matching!()).returns(40).in_any_order(),
            baz::Fn::each_call(matching!()).returns(2).in_any_order(),
        ]);

        let result = sum(&mock).await;

        assert_eq!(42, result);
    }
}

mod where_bounds {
    use super::*;

    #[entrait(Sum, async_trait = true)]
    async fn sum<A>(a: &A) -> i32
    where
        A: Bar + Baz,
    {
        a.bar().await + a.baz().await
    }

    #[tokio::test]
    async fn test_mock() {
        assert_eq!(
            42,
            sum(&mock([
                bar::Fn::each_call(matching!()).returns(40).in_any_order(),
                baz::Fn::each_call(matching!()).returns(2).in_any_order(),
            ]))
            .await
        );
    }
}

mod impl_trait_bounds {
    use super::*;

    #[entrait(Sum, async_trait = true)]
    async fn sum(a: &(impl Bar + Baz)) -> i32 {
        a.bar().await + a.baz().await
    }

    #[tokio::test]
    async fn test_mock() {
        assert_eq!(
            42,
            sum(&mock([
                bar::Fn::each_call(matching!()).returns(40).in_any_order(),
                baz::Fn::each_call(matching!()).returns(2).in_any_order(),
            ]))
            .await
        );
    }
}
