package com.musicfrog.infiltrator

/** Component fixture exposing only the Messenger keys consumed by the isolated client adapter. */
class MihomoControlService {
    companion object {
        const val REQUEST = 41
        const val RESPONSE = 42
        const val CANCEL_START_REQUEST = 43
        const val ACK_START_REQUEST = 44
        const val CANCEL_OWNER_REQUEST = 45
        const val PROTOCOL_VERSION = "protocol_version"
        const val CLIENT_SESSION_ID = "client_session_id"
        const val EXPECTED_OWNER_SESSION_ID = "expected_owner_session_id"
        const val OWNER_SESSION_ID = "owner_session_id"
        const val REQUEST_ID = "request_id"
        const val REQUEST_DEADLINE_ELAPSED_REALTIME = "request_deadline_elapsed_realtime"
        const val REQUEST_VALUE = "request"
        const val RESPONSE_VALUE = "response"
        const val RESPONSE_PAYLOAD_KIND = "response_payload_kind"
        const val STATUS_CODE = "status_code"
        const val STATUS_MESSAGE = "status_message"
        const val OWNER_OPERATION_JSON = "owner_operation_json"
        const val PROFILE_AGE_CREDENTIAL_METADATA_JSON = "profile_age_credential_metadata_json"
        const val PROFILE_AGE_CREDENTIAL_PRIVATE_KEY_BYTES = "profile_age_credential_private_key_bytes"
        const val OWNER_DATA_REVISION = "owner_data_revision"
        const val OWNER_DATA_GENERATION = "owner_data_generation"
        const val OWNER_OPERATION_DATA_FACTS_KIND = "owner_operation_data_facts_kind"
        const val OWNER_OPERATION_DATA_DOMAIN = "owner_operation_data_domain"
        const val OWNER_OPERATION_DATA_SNAPSHOT_JSON = "owner_operation_data_snapshot_json"
        const val OWNER_OPERATION_RESULT_JSON = "owner_operation_result_json"
        const val PRODUCT_COMMAND_JSON = "product_command_json"
        const val PRODUCT_COMMAND_RESULT_JSON = "product_command_result_json"
        const val SURFACE_SNAPSHOT_JSON = "surface_snapshot_json"
        const val VPN_CONFIGURATION = "vpn_configuration"
        const val VPN_CONFIGURATION_OUTCOME = "vpn_configuration_outcome"
        const val VPN_AUTHORIZATION_STATE = "vpn_authorization_state"
        const val VPN_START_OUTCOME = "vpn_start_outcome"
        const val VPN_OWNER_SNAPSHOT_JSON = "vpn_owner_snapshot_json"
        const val VPN_STOP_SNAPSHOT_JSON = "vpn_stop_snapshot_json"
        const val VPN_STOP_TARGET_CODE = "vpn_stop_target_code"
        const val VPN_STOP_GENERATION = "vpn_stop_generation"
        const val CORE_GENERATION = "core_generation"
        const val CORE_READBACK_KIND = "core_readback_kind"
        const val CORE_SNAPSHOT_LIFECYCLE = "core_snapshot_lifecycle"
        const val CORE_SNAPSHOT_GENERATION = "core_snapshot_generation"
        const val CORE_SNAPSHOT_SESSION_TOKEN = "core_snapshot_session_token"
        const val CORE_SNAPSHOT_REVISION = "core_snapshot_revision"
        const val CORE_SNAPSHOT_IDENTITY_JSON = "core_snapshot_identity_json"
        const val CORE_SNAPSHOT_FAILURE_JSON = "core_snapshot_failure_json"
    }
}
