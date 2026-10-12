use super::*;

#[test]
fn profile_controller_configuration_requires_a_fixed_loopback_socket() {
    let configuration = SingBoxControllerConfiguration::from_profile_json(
        br#"{"experimental":{"clash_api":{"external_controller":"127.0.0.1:9090"}}}"#,
    )
    .expect("configured loopback Clash API");

    assert_eq!(configuration.address(), "127.0.0.1:9090".parse().unwrap());
    assert_eq!(configuration.endpoint(), "http://127.0.0.1:9090");
    assert_eq!(
        configuration.authorization(),
        &SingBoxControllerAuthorization::NoSecret
    );

    let missing = SingBoxControllerConfiguration::from_profile_json(br#"{"outbounds":[]}"#);
    assert_eq!(
        missing,
        Err(SingBoxControllerConfigurationError::ClashApiDisabled)
    );
    let public_bind = SingBoxControllerConfiguration::from_profile_json(
        br#"{"experimental":{"clash_api":{"external_controller":"0.0.0.0:9090"}}}"#,
    );
    assert_eq!(
        public_bind,
        Err(SingBoxControllerConfigurationError::ExternalControllerNotLoopback)
    );
    let ephemeral = SingBoxControllerConfiguration::from_profile_json(
        br#"{"experimental":{"clash_api":{"external_controller":"127.0.0.1:0"}}}"#,
    );
    assert_eq!(
        ephemeral,
        Err(SingBoxControllerConfigurationError::ExternalControllerHasEphemeralPort)
    );
}

#[test]
fn configured_bearer_secret_is_typed_and_debug_output_redacts_it() {
    let configuration = SingBoxControllerConfiguration::from_profile_json(
        br#"{"experimental":{"clash_api":{"external_controller":"[::1]:9090","secret":"private-token"}}}"#,
    )
    .expect("configured loopback Clash API with bearer secret");

    assert_eq!(configuration.endpoint(), "http://[::1]:9090");
    assert_eq!(
        configuration.authorization(),
        &SingBoxControllerAuthorization::BearerToken("private-token".to_owned())
    );
    let debug = format!("{:?}", configuration);
    assert!(!debug.contains("private-token"));
    assert!(debug.contains("redacted"));

    let invalid_secret = SingBoxControllerConfiguration::from_profile_json(
        br#"{"experimental":{"clash_api":{"external_controller":"127.0.0.1:9090","secret":7}}}"#,
    );
    assert_eq!(
        invalid_secret,
        Err(SingBoxControllerConfigurationError::InvalidSecret)
    );
}
