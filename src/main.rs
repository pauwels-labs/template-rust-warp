use handlebars::Handlebars;
use redact_config::Configurator;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashMap, sync::Arc, thread, time};
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

fn sleep() -> impl Filter<Extract = (WithTemplate<Value>,), Error = warp::Rejection> + Copy {
    warp::path!("sleep")
        .and(warp::query::<HashMap<String, String>>())
        .map(|p: HashMap<String, String>| match p.get("length") {
            Some(length) => length.to_owned(),
            None => "1000".to_owned(),
        })
        .and_then(move |length: String| async move {
            match length.parse::<u64>() {
                Ok(length) => {
                    if length > 10000 {
                        Ok::<_, Rejection>(WithTemplate {
                            name: "index",
                            value: json!({ "sleep-error-msg": "Length must be a positive integer between 0 and 10000"}),
                        })
                    } else {
                    let simulated_load_time = time::Duration::from_millis(length);
                    let time_taken_message = format!("Successfully slept {length} milliseconds");
                    thread::sleep(simulated_load_time);
                    Ok::<_, Rejection>(WithTemplate {
                        name: "index",
                        value: json!({ "sleep-success-msg": time_taken_message }),
                    })
                }
                }
                Err(_) => {
                    Ok::<_, Rejection>(WithTemplate {
                        name: "index",
                        value: json!({ "sleep-error-msg": "Length must be a positive integer between 0 and 10000"}),
                    })
                }
            }
        })
}

#[cfg(test)]
mod test {
    use super::sleep;
    use serde_json::json;
    use std::time::{Duration, Instant};
    
    #[tokio::test]
    async fn test_sleep_default() {
        let filter = sleep();
        let before = Instant::now();
        let value = warp::test::request()
            .path("/sleep")
            .filter(&filter)
            .await
            .unwrap();
        let after = before.elapsed();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "sleep-success-msg": "Successfully slept 1000 milliseconds" })
        );
        assert!(after >= Duration::from_millis(1000));
    }

    #[tokio::test]
    async fn test_sleep_custom_length() {
        let filter = sleep();
        let before = Instant::now();
        let value = warp::test::request()
            .path("/sleep?length=3000")
            .filter(&filter)
            .await
            .unwrap();
        let after = before.elapsed();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "sleep-success-msg": "Successfully slept 3000 milliseconds" })
        );
        assert!(after >= Duration::from_millis(3000));
    }

    #[tokio::test]
    async fn test_sleep_below_edge_case() {
        let filter = sleep();
        let before = Instant::now();
        let value = warp::test::request()
            .path("/sleep?length=9999")
            .filter(&filter)
            .await
            .unwrap();
        let after = before.elapsed();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "sleep-success-msg": "Successfully slept 9999 milliseconds" })
        );
        assert!(after >= Duration::from_millis(9999));
    }

    #[tokio::test]
    async fn test_sleep_at_edge_case() {
        let filter = sleep();
        let before = Instant::now();
        let value = warp::test::request()
            .path("/sleep?length=10000")
            .filter(&filter)
            .await
            .unwrap();
        let after = before.elapsed();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "sleep-success-msg": "Successfully slept 10000 milliseconds" })
        );
        assert!(after >= Duration::from_millis(10000));
    }

    #[tokio::test]
    async fn test_sleep_non_integer_length() {
        let filter = sleep();
        let value = warp::test::request()
            .path("/sleep?length=string")
            .filter(&filter)
            .await
            .unwrap();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "sleep-error-msg": "Length must be a positive integer between 0 and 10000"})
        );
    }

    #[tokio::test]
    async fn test_sleep_above_edge_case() {
        let filter = sleep();
        let value = warp::test::request()
            .path("/sleep?length=10001")
            .filter(&filter)
            .await
            .unwrap();
        assert_eq!(value.name, "index");
        assert_eq!(
            value.value,
            json!({ "sleep-error-msg": "Length must be a positive integer between 0 and 10000"})
        );
    }
}

#[tokio::main]
async fn main() {
    let config = redact_config::new("WEBSITE").unwrap();

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

    let sleep_route = sleep().map(handlebars.clone());

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
                slack_body_map.insert("text", format!(r#"<!channel> {}"#, msg));

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
        .or(message_route)
        .or(sleep_route);

    warp::serve(static_routes).run(([0, 0, 0, 0], 8080)).await;
}
