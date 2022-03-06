use entrait::entrait;

#[entrait(Bar, async_trait = true, mockable = true)]
#[allow(dead_code)]
async fn bar<A>(_: &A) -> u32 {
    unimplemented!()
}

#[entrait(Baz, async_trait = true, mockable = true)]
#[allow(dead_code)]
async fn baz<A>(_: &A) -> u32 {
    unimplemented!()
}

mod inline_bounds {
    use super::*;

    #[entrait(Sum, async_trait = true, mock_deps_as=BarBaz)]
    async fn sum<A: Bar + Baz>(a: &A) -> u32 {
        a.bar().await + a.baz().await
    }

    #[tokio::test]
    async fn test_mock() {
        let mut mock_bar_baz = MockBarBaz::new();
        mock_bar_baz.expect_bar().returning(|| 40);
        mock_bar_baz.expect_baz().returning(|| 2);

        let result = sum(&mock_bar_baz).await;

        assert_eq!(42, result);
    }
}

mod where_bounds {
    use super::*;

    #[entrait(Sum, async_trait = true, mock_deps_as=BarBaz)]
    async fn sum<A>(a: &A) -> u32
    where
        A: Bar + Baz,
    {
        a.bar().await + a.baz().await
    }

    #[tokio::test]
    async fn test_mock() {
        let mut mock_bar_baz = MockBarBaz::new();
        mock_bar_baz.expect_bar().returning(|| 40);
        mock_bar_baz.expect_baz().returning(|| 2);

        let result = sum(&mock_bar_baz).await;

        assert_eq!(42, result);
    }
}

mod impl_trait_bounds {
    use super::*;

    #[entrait(Sum, async_trait = true, mock_deps_as=BarBaz)]
    async fn sum(a: &(impl Bar + Baz)) -> u32 {
        a.bar().await + a.baz().await
    }

    #[tokio::test]
    async fn test_mock() {
        let mut mock_bar_baz = MockBarBaz::new();
        mock_bar_baz.expect_bar().returning(|| 40);
        mock_bar_baz.expect_baz().returning(|| 2);

        let result = sum(&mock_bar_baz).await;

        assert_eq!(42, result);
    }
}
