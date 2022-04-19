#![feature(generic_associated_types)]

use entrait::*;
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

#[entrait(Authenticate for Runtime, async_trait=true, unimock = true)]
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

#[entrait(FetchUser for Runtime, unimock = true)]
fn fetch_user(_: &impl Any, _id: u32) -> Option<User> {
    Some(User {
        username: "name".into(),
        hash: "h4sh".into(),
    })
}

#[entrait(VerifyPassword for Runtime, unimock = true)]
fn verify_password(_: &impl Any, _password: &str, _hash: &str) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_username() {
        let username = get_username(
            &mock(authenticate::Fn, |each| {
                each.call(matching!(_, _)).returns(Ok(User {
                    username: "foobar".into(),
                    hash: "h4sh".into(),
                }));
            }),
            42,
            "pw",
        )
        .await
        .unwrap();
        assert_eq!("foobar", username);
    }

    #[tokio::test]
    async fn test_authenticate() {
        let mocks = mock(fetch_user::Fn, |each| {
            each.call(matching!(42)).once().returns(Some(User {
                username: "foobar".into(),
                hash: "h4sh".into(),
            }));
        })
        .also(verify_password::Fn, |each| {
            each.call(matching!("pw", "h4sh")).once().returns(true);
        });

        let user = authenticate(&mocks, 42, "pw").await.unwrap();
        assert_eq!("foobar", user.username);
    }
}
