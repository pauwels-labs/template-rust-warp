use handlebars::Handlebars;
use rand::distributions::Alphanumeric;
use rand::Rng;
use redact_config::Configurator;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha512};
use std::{collections::HashMap, net::IpAddr, path::Path, str::FromStr, sync::Arc};
use warp::{Filter, Rejection};

#[derive(Deserialize, Serialize)]
struct MyObject {
    length: u64,
}
struct WithTemplate<T: Serialize> {
    name: &'static str,
    value: T,
}

fn render<T>(template: WithTemplate<T>, hbs: Arc<Handlebars>) -> impl warp::Reply
where
    T: Serialize,
{
    let render = hbs
        .render(template.name, &template.value)
        .unwrap_or_else(|err| err.to_string());
    warp::reply::html(render)
}

fn hash() -> impl Filter<Extract = (WithTemplate<Value>,), Error = warp::Rejection> + Copy {
    warp::path!("hash")
        .and(warp::query::<HashMap<String, String>>())
        .map(|p: HashMap<String, String>| match p.get("amount") {
            Some(amount) => amount.to_owned(),
            None => "1000".to_owned(),
        })
        .and_then(move |amount: String| async move {
            match amount.parse::<u64>() {
                Ok(amount) => {
                    if amount > 10000 {
                        Ok::<_, Rejection>(WithTemplate {
                            name: "index",
                            value: json!({ "hash-error-msg": "Amount must be a positive integer between 0 and 10,000"}),
                        })
                    } else {
                        let mut map: HashMap<u64, String> = HashMap::new();
                        for n in 0..amount {
                            let mut hasher = Sha512::new();
                            let rand_string: String = rand::thread_rng()
                                .sample_iter(&Alphanumeric)
                                .take(64)
                                .map(char::from)
                                .collect();
                            hasher.update(rand_string);
                            let result = format!("{:x}", hasher.finalize());
                            map.insert(n, result);
                        }
                        let hash_message = format!("Successfully hashed {amount} times");
                        Ok::<_, Rejection>(WithTemplate {
                            name: "index",
                            value: json!({ "hash-success-msg": hash_message }),
                        })
                }
                }
                Err(_) => {
                    Ok::<_, Rejection>(WithTemplate {
                        name: "index",
                        value: json!({ "hash-error-msg": "Amount must be a positive integer between 0 and 10,000"}),
                    })
                }
            }
        })
}

#[cfg(test)]
mod test {
    use super::hash;
    use serde_json::json;
    use std::time::{Duration, Instant};

    #[tokio::test]
    async fn test_hash_default() {
        let filter = hash();
        let value = warp::test::request()
            .path("/hash")
            .filter(&filter)
            .await
            .unwrap();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "hash-success-msg": "Successfully hashed 1000 times" })
        );
    }

    #[tokio::test]
    async fn test_hash_custom_amount() {
        let filter = hash();
        let value = warp::test::request()
            .path("/hash?amount=3000")
            .filter(&filter)
            .await
            .unwrap();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "hash-success-msg": "Successfully hashed 3000 times" })
        );
    }

    #[tokio::test]
    async fn test_hash_below_edge_case() {
        let filter = hash();
        let value = warp::test::request()
            .path("/hash?amount=9999")
            .filter(&filter)
            .await
            .unwrap();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "hash-success-msg": "Successfully hashed 9999 times" })
        );
    }

    #[tokio::test]
    async fn test_hash_at_edge_case() {
        let filter = hash();
        let value = warp::test::request()
            .path("/hash?amount=10000")
            .filter(&filter)
            .await
            .unwrap();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "hash-success-msg": "Successfully hashed 10000 times" })
        );
    }

    #[tokio::test]
    async fn test_hash_non_integer_amount() {
        let filter = hash();
        let value = warp::test::request()
            .path("/hash?amount=string")
            .filter(&filter)
            .await
            .unwrap();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "hash-error-msg": "Amount must be a positive integer between 0 and 10,000"})
        );
    }

    #[tokio::test]
    async fn test_hash_above_edge_case() {
        let filter = hash();
        let value = warp::test::request()
            .path("/hash?amount=10001")
            .filter(&filter)
            .await
            .unwrap();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "hash-error-msg": "Amount must be a positive integer between 0 and 10,000"})
        );
    }
}

#[tokio::main]
async fn main() {
    // let binding = "0.0.0.0:9184".parse().unwrap();
    // let exporter = prometheus_exporter::start(binding).unwrap();
    // let guard = exporter.wait_request();
    // drop(guard);

    let config_path = if Path::new("/etc/homepage/config").is_dir() {
        "/etc/homepage/config".to_owned()
    } else {
        "./config".to_owned()
    };
    let config = redact_config::new(&config_path, "APPCFG").unwrap();

    let mut hb = Handlebars::new();
    hb.register_template_file("index", "./static/index.html")
        .unwrap();

    let hb = Arc::new(hb);

    let handlebars = move |with_template| render(with_template, hb.clone());

    let index_route = warp::path::end()
        .map(|| WithTemplate {
            name: "index",
            value: json!({}),
        })
        .map(handlebars.clone());

    let expandable_route =
        warp::path!("expandable").and(warp::fs::file("./static/expandable.html"));
    let scalable_route = warp::path!("scalable").and(warp::fs::file("./static/scalable.html"));
    let highly_available_route =
        warp::path!("highly-available").and(warp::fs::file("./static/highly-available.html"));
    let full_stack_route =
        warp::path!("full-stack").and(warp::fs::file("./static/full-stack.html"));
    let full_service_route =
        warp::path!("full-service").and(warp::fs::file("./static/full-service.html"));
    let cloud_route = warp::path!("cloud").and(warp::fs::file("./static/cloud.html"));
    let css_routes = warp::path!("css" / ..).and(warp::fs::dir("./static/css"));

    let slack_webhook_url = config.get_str("slack.webhook").unwrap();

    // let hash_route = hash().map(handlebars.clone());

    let message_route = warp::path!("message")
        .and(warp::post())
        .and(warp::body::content_length_limit(1024 * 20))
        .and(warp::body::form())
        .map(|form_map: HashMap<String, String>| {
            form_map
                .get("message")
                .map(|m| m.to_string())
                .unwrap_or("".to_string())
        })
        .and_then(move |msg: String| {
            let slack_webhook_url = slack_webhook_url.clone();
            async move {
                if msg.len() <= 0 {
                    return Ok::<_, std::convert::Infallible>(WithTemplate {
                        name: "index",
                        value: json!({ "msg": "", "error-msg": "Don't forget to write a message"}),
                    });
                }

                let mut slack_body_map = HashMap::new();
                slack_body_map.insert("text", format!(r#"<!channel> {}"#, format!("homepage: {}", msg)));

                Client::new()
                    .post(&slack_webhook_url)
                    .json(&slack_body_map)
                    .send()
                    .await
                .map_or_else(|_| {
                Ok(WithTemplate {
                    name: "index",
                    value: json!({ "msg": msg, "error-msg": "An error occurred while sending, try again in a little bit" }),
                })
                }, |_| {
                Ok(WithTemplate {
                    name: "index",
                    value: json!({ "success-msg": "Message received, expect a response within 24 hours" }),
                })
                })
            }
        })
        .map(handlebars.clone());

    let static_routes = index_route
        .or(expandable_route)
        .or(scalable_route)
        .or(highly_available_route)
        .or(full_stack_route)
        .or(full_service_route)
        .or(cloud_route)
        .or(css_routes)
        .or(message_route);
    //.or(hash_route);

    println!("Starting server listening [::0]:8080");
    let addr = IpAddr::from_str("::0").unwrap();
    warp::serve(static_routes).run((addr, 8080)).await;
}
