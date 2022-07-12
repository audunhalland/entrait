use entrait::entrait;

use feignhttp::get;

#[entrait(NoDeps, no_deps)]
fn no_deps(_a: i32, _b: i32) {}

#[entrait(CallMyApi, no_deps, async_trait)]
#[get("https://my.api.org/api/{param}")]
async fn call_my_api(#[path] param: String) -> feignhttp::Result<String> {}
