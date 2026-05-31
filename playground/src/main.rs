use std::sync::Arc;

use axum::{routing::post, Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use zen_engine::model::DecisionContent;
use zen_engine::{DecisionEngine, EvaluationOptions};
use zen_expression::variable::Variable;

#[derive(Deserialize)]
struct EvalRequest {
    jdm: Value,
    context: Value,
    #[serde(default)]
    trace: bool,
}

// zen Variable uses Rc internally and is !Send, so the whole evaluation
// (parse -> evaluate -> serialize) runs inside spawn_blocking on a
// current-thread runtime. Only Send serde_json::Value crosses the boundary.
async fn evaluate(Json(req): Json<EvalRequest>) -> Json<Value> {
    let EvalRequest { jdm, context, trace } = req;

    let out = tokio::task::spawn_blocking(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to build runtime");

        rt.block_on(async move {
            let content: DecisionContent = match serde_json::from_value(jdm) {
                Ok(c) => c,
                Err(e) => return json!({ "ok": false, "error": format!("Invalid JDM: {e}") }),
            };

            let engine = DecisionEngine::default();
            let decision = engine.create_decision(Arc::new(content));
            let ctx = Variable::from(context);
            let opts = EvaluationOptions { trace, max_depth: 10 };

            match decision.evaluate_with_opts(ctx, opts).await {
                Ok(resp) => {
                    let mut v = serde_json::to_value(&resp).unwrap_or_else(|_| json!({}));
                    if let Value::Object(ref mut m) = v {
                        m.insert("ok".to_string(), Value::Bool(true));
                    }
                    v
                }
                Err(e) => json!({ "ok": false, "error": e.to_string() }),
            }
        })
    })
    .await
    .unwrap_or_else(|e| json!({ "ok": false, "error": format!("evaluation task failed: {e}") }));

    Json(out)
}

#[tokio::main]
async fn main() {
    let static_dir = concat!(env!("CARGO_MANIFEST_DIR"), "/static");

    let app = Router::new()
        .route("/api/evaluate", post(evaluate))
        .fallback_service(ServeDir::new(static_dir))
        .layer(CorsLayer::permissive());

    let addr = "127.0.0.1:3000";
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind");
    println!("ZEN playground running -> http://{addr}");
    axum::serve(listener, app).await.expect("server error");
}
