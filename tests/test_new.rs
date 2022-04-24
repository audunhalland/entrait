#![feature(generic_associated_types)]

use entrait::unimock::*;
use std::any::Any;
use unimock::*;

struct Runtime;

type Error = ();

#[derive(Clone)]
pub struct User {
    username: String,
    hash: String,
}

async fn _wire_everything() -> Result<String, Error> {
    let runtime = Runtime;
    get_username(&runtime, 42, "password").await
}

async fn get_username(rt: &impl Authenticate, id: u32, password: &str) -> Result<String, Error> {
    let user = rt.authenticate(id, password).await?;
    Ok(user.username)
}

#[entrait(Authenticate for Runtime, async_trait=true)]
async fn authenticate(
    deps: &(impl FetchUser + VerifyPassword),
    id: u32,
    password: &str,
) -> Result<User, Error> {
    let user = deps.fetch_user(id).ok_or(())?;
    if deps.verify_password(password, &user.hash) {
        Ok(user)
    } else {
        Err(())
    }
}

#[entrait(FetchUser for Runtime)]
fn fetch_user(_: &impl Any, _id: u32) -> Option<User> {
    Some(User {
        username: "name".into(),
        hash: "h4sh".into(),
    })
}

#[entrait(VerifyPassword for Runtime)]
fn verify_password(_: &impl Any, _password: &str, _hash: &str) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_username() {
        let username = get_username(
            &mock(Some(authenticate::Fn::stub(|each| {
                each.call(matching!(_, _)).returns(Ok(User {
                    username: "foobar".into(),
                    hash: "h4sh".into(),
                }));
            }))),
            42,
            "pw",
        )
        .await
        .unwrap();
        assert_eq!("foobar", username);
    }

    #[tokio::test]
    async fn test_authenticate() {
        let mocks = mock([
            fetch_user::Fn::each_call(matching!(42))
                .returns(Some(User {
                    username: "foobar".into(),
                    hash: "h4sh".into(),
                }))
                .in_any_order(),
            verify_password::Fn::each_call(matching!("pw", "h4sh"))
                .returns(true)
                .once()
                .in_any_order(),
        ]);

        let user = authenticate(&mocks, 42, "pw").await.unwrap();
        assert_eq!("foobar", user.username);
    }

    #[tokio::test]
    async fn test_full_spy() {
        let user = authenticate(&spy(None), 42, "pw").await.unwrap();

        assert_eq!("name", user.username);
    }
}
