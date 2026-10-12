//! ide-shell-protocol unit tests — vim-like RPC facade.

use crate::*;

#[tokio::test]
async fn test_open_buffer_returns_id() {
    let k = Kernel::new();
    let id = k.open_buffer("foo.rs").await;
    assert!(id > 0);
    let buf = k.get_buffer(id).await.unwrap();
    assert_eq!(buf.name, "foo.rs");
    assert_eq!(buf.lines.len(), 1);
}

#[tokio::test]
async fn test_set_buffer_lines_changes_content() {
    let k = Kernel::new();
    let id = k.open_buffer("test").await;
    k.set_buffer_lines(id, vec!["hello".into(), "world".into()]).await.unwrap();
    let buf = k.get_buffer(id).await.unwrap();
    assert_eq!(buf.lines, vec!["hello", "world"]);
    assert!(buf.dirty);
}

#[tokio::test]
async fn test_key_event_insert_mode_typing() {
    let k = Kernel::new();
    let id = k.open_buffer("type-test").await;
    // 进 INSERT
    k.key_event(id, "i", false, false).await.unwrap();
    assert_eq!(k.get_buffer(id).await.unwrap().mode, Mode::Insert);
    // 输 "hi"
    k.key_event(id, "h", false, false).await.unwrap();
    k.key_event(id, "i", false, false).await.unwrap();
    let buf = k.get_buffer(id).await.unwrap();
    assert_eq!(buf.lines[0], "hi");
    assert_eq!(buf.cursor.col, 2);
}

#[tokio::test]
async fn test_enter_newline_indent_after_brace() {
    let k = Kernel::new();
    let id = k.open_buffer("rust").await;
    k.set_buffer_lines(id, vec!["fn main() {".into()]).await.unwrap();
    // 用 set_cursor 把 cursor 设到行末
    k.set_cursor(id, 0, 11).await.unwrap();
    // 进 INSERT, 按 Enter
    k.key_event(id, "i", false, false).await.unwrap();
    k.key_event(id, "Enter", false, false).await.unwrap();
    let buf = k.get_buffer(id).await.unwrap();
    assert_eq!(buf.lines.len(), 2, "expected 2 lines, got {:?}", buf.lines);
    assert_eq!(buf.lines[1], "  ", "expected indent, got {:?}", buf.lines[1]);
    assert_eq!(buf.cursor.row, 1);
    assert_eq!(buf.cursor.col, 2);
}

#[tokio::test]
async fn test_rpc_open_buffer() {
    let k = Kernel::new();
    let req = r#"{"jsonrpc":"2.0","id":1,"method":"open_buffer","params":{"name":"rpc.rs"}}"#;
    let resp = k.handle_rpc(req).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["result"]["id"], 1);
}

#[tokio::test]
async fn test_rpc_insert_and_get_buffer() {
    let k = Kernel::new();
    // open buffer via RPC
    let resp = k.handle_rpc(r#"{"jsonrpc":"2.0","id":1,"method":"open_buffer","params":{"name":"x"}}"#).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    let id = v["result"]["id"].as_u64().unwrap();

    // insert via RPC
    let resp = k.handle_rpc(&format!(
        r#"{{"jsonrpc":"2.0","id":2,"method":"insert_at_cursor","params":{{"id":{id},"text":"hello\n"}}}}"#
    )).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["result"]["ok"], true);

    // get_buffer
    let resp = k.handle_rpc(&format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"get_buffer","params":{{"id":{id}}}}}"#
    )).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["result"]["lines"][0], "hello");
}

#[tokio::test]
async fn test_rpc_method_not_found() {
    let k = Kernel::new();
    let resp = k.handle_rpc(r#"{"jsonrpc":"2.0","id":1,"method":"bogus","params":{}}"#).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert!(v["error"].is_object());
    assert!(v["error"]["message"].as_str().unwrap().contains("unknown method"));
}

#[tokio::test]
async fn test_rpc_notification_returns_none() {
    let k = Kernel::new();
    // id is null = notification
    let resp = k.handle_rpc(r#"{"jsonrpc":"2.0","id":null,"method":"open_buffer","params":{"name":"n"}}"#).await;
    assert!(resp.is_none());
}

#[tokio::test]
async fn test_kernel_info() {
    let k = Kernel::new();
    let resp = k.handle_rpc(r#"{"jsonrpc":"2.0","id":1,"method":"kernel_info","params":{}}"#).await.unwrap();
    let v: serde_json::Value = serde_json::from_str(&resp).unwrap();
    assert_eq!(v["result"]["name"], "ide-shell-protocol");
    assert!(v["result"]["api"].as_array().unwrap().len() >= 15);
}

#[tokio::test]
async fn test_active_buffer_tracking() {
    let k = Kernel::new();
    let id1 = k.open_buffer("a").await;
    let id2 = k.open_buffer("b").await;
    assert_eq!(k.active_buffer_id().await, id2);
    k.set_active_buffer(id1).await.unwrap();
    assert_eq!(k.active_buffer_id().await, id1);
    let ids = k.list_buffers().await;
    assert!(ids.contains(&id1) && ids.contains(&id2));
}

struct MockProvider {
    name: String,
}

#[async_trait::async_trait]
impl AiProvider for MockProvider {
    fn name(&self) -> &str { &self.name }
    async fn complete(&self, req: CompletionRequest) -> Result<Vec<CompletionItem>, String> {
        Ok(vec![CompletionItem {
            label: req.prefix.clone(),
            insert_text: format!("{}_completed", req.prefix),
            kind: "test".into(),
            detail: Some("mock provider".into()),
            documentation: None,
        }])
    }
    async fn edit(&self, _req: EditRequest) -> Result<EditResponse, String> {
        Ok(EditResponse { new_text: "// mocked edit".into(), explanation: Some("mock".into()) })
    }
    async fn chat(&self, _req: ChatRequest) -> Result<ChatResponse, String> {
        Ok(ChatResponse {
            message: ChatMessage { role: "assistant".into(), content: "mocked reply".into(), name: None, tool_call_id: None },
            tool_calls: vec![],
            finish_reason: Some("stop".into()),
        })
    }
}

#[tokio::test]
async fn test_provider_register_and_complete() {
    let k = Kernel::new();
    k.register_provider("mock", std::sync::Arc::new(MockProvider { name: "mock".into() })).await;
    let id = k.open_buffer("test.rs").await;
    k.set_buffer_lines(id, vec!["hel".into()]).await.unwrap();
    // Move cursor to end of "hel"
    k.key_event(id, "i", false, false).await.unwrap();
    k.key_event(id, "ArrowRight", false, false).await.unwrap();
    k.key_event(id, "ArrowRight", false, false).await.unwrap();
    k.key_event(id, "ArrowRight", false, false).await.unwrap();
    let result = k.complete_at_cursor(id).await.unwrap();
    let items = result["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["kind"], "test");
}

#[tokio::test]
async fn test_provider_chat() {
    let k = Kernel::new();
    k.register_provider("mock", std::sync::Arc::new(MockProvider { name: "mock".into() })).await;
    let resp = k.chat(ChatRequest {
        buffer_id: None,
        messages: vec![ChatMessage { role: "user".into(), content: "hi".into(), name: None, tool_call_id: None }],
        tools: vec![],
    }).await.unwrap();
    assert_eq!(resp.message.content, "mocked reply");
    assert_eq!(resp.finish_reason.unwrap(), "stop");
}

#[tokio::test]
async fn test_event_subscription() {
    let k = Kernel::new();
    let mut rx = k.subscribe().await;
    k.open_buffer("event-test").await;
    let ev = rx.recv().await.unwrap();
    match ev {
        Event::BufferOpened { name, .. } => assert_eq!(name, "event-test"),
        _ => panic!("expected BufferOpened, got {:?}", ev),
    }
}

#[tokio::test]
async fn test_provider_unregister() {
    let k = Kernel::new();
    k.register_provider("mock", std::sync::Arc::new(MockProvider { name: "mock".into() })).await;
    assert_eq!(k.list_providers().await, vec!["mock"]);
    k.unregister_provider("mock").await;
    assert_eq!(k.list_providers().await, vec![] as Vec<String>);
}
