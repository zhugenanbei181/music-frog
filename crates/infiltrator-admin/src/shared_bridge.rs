use serde::{Deserialize, Serialize};
use serde_json::Value;

use infiltrator_shared::error_codes::{get_localized_error, InfiltratorErrorCode, StructuredError};
use infiltrator_core::redact::redact_line;

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BridgeRequest {
    pub intent: String,
    pub payload: Option<Value>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct BridgeResponse {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<StructuredError>,
}

pub struct AdminSharedBridge;

impl AdminSharedBridge {
    pub fn handle_intent(req: &BridgeRequest, lang: &str) -> BridgeResponse {
        match req.intent.as_str() {
            "StartProxy" | "StopProxy" | "SwitchProfile" | "CheckUpdates" | "SyncWebDav" | "InspectDiagnostics" => {
                BridgeResponse {
                    success: true,
                    data: Some(serde_json::json!({ "status": "success", "intent": req.intent })),
                    error: None,
                }
            }
            _ => {
                let code = InfiltratorErrorCode::Internal(format!("Unknown intent: {}", req.intent));
                let mut error = get_localized_error(&code, lang);
                error.message = redact_line(&error.message, &[]);
                
                BridgeResponse {
                    success: false,
                    data: None,
                    error: Some(error),
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_intent() {
        let req = BridgeRequest {
            intent: "StartProxy".to_string(),
            payload: None,
        };
        let resp = AdminSharedBridge::handle_intent(&req, "en-US");
        assert!(resp.success);
        assert!(resp.error.is_none());
        assert_eq!(resp.data.unwrap()["intent"], "StartProxy");
    }

    #[test]
    fn test_unknown_intent_en() {
        let req = BridgeRequest {
            intent: "ExplodeProxy".to_string(),
            payload: None,
        };
        let resp = AdminSharedBridge::handle_intent(&req, "en-US");
        assert!(!resp.success);
        assert!(resp.data.is_none());
        
        let err = resp.error.unwrap();
        match err.code {
            InfiltratorErrorCode::Internal(msg) => {
                assert!(msg.contains("ExplodeProxy"));
            }
            _ => panic!("Expected Internal error"),
        }
    }

    #[test]
    fn test_unknown_intent_zh() {
        let req = BridgeRequest {
            intent: "ExplodeProxy".to_string(),
            payload: None,
        };
        let resp = AdminSharedBridge::handle_intent(&req, "zh-CN");
        assert!(!resp.success);
        assert!(resp.data.is_none());
        
        let err = resp.error.unwrap();
        match err.code {
            InfiltratorErrorCode::Internal(msg) => {
                assert!(msg.contains("ExplodeProxy"));
            }
            _ => panic!("Expected Internal error"),
        }
    }
}
