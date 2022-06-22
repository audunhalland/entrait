// BUG: Unimock unmock is broken (it passes one parameter too much)
// use entrait::unimock::entrait;

use entrait::entrait;

use feignhttp::get;

#[entrait(NoDeps, no_deps)]
fn no_deps(a: i32, b: i32) {}

#[entrait(CallMyApi, no_deps, async_trait)]
#[get("https://my.api.org/api")]
async fn call_my_api() -> feignhttp::Result<String> {}
