// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.
// SPDX-License-Identifier: MIT

use std::env;

use digital_twin_model::trailer_v1;
use digital_twin_providers_common::constants::chariott::{
    INVEHICLE_DIGITAL_TWIN_SERVICE_COMMUNICATION_KIND,
    INVEHICLE_DIGITAL_TWIN_SERVICE_COMMUNICATION_REFERENCE, INVEHICLE_DIGITAL_TWIN_SERVICE_NAME,
    INVEHICLE_DIGITAL_TWIN_SERVICE_NAMESPACE, INVEHICLE_DIGITAL_TWIN_SERVICE_VERSION,
};
use digital_twin_providers_common::constants::{
    constraint_type, digital_twin_operation, digital_twin_protocol,
};
use digital_twin_providers_common::utils::{
    discover_digital_twin_provider_using_ibeji, discover_service_using_chariott, get_uri,
};
use env_logger::{Builder, Target};
use interfaces::invehicle_digital_twin::v1::EndpointInfo;
use interfaces::module::managed_subscribe::v1::managed_subscribe_client::ManagedSubscribeClient;
use interfaces::module::managed_subscribe::v1::{
    Constraint, SubscriptionInfoRequest, SubscriptionInfoResponse,
};
use log::{debug, info, LevelFilter};
use paho_mqtt as mqtt;
use tokio::signal;
use tokio::task::JoinHandle;
use tokio::time::{sleep, Duration};
use tonic::{Request, Status};
use uuid::Uuid;

use futures::{future, FutureExt}; // 0.3.5

const FREQUENCY_MS_FLAG: &str = "freq_ms=";
const MQTT_CLIENT_ID: &str = "smart-trailer-consumer";

// TODO: These could be added in configuration
const CHARIOTT_SERVICE_DISCOVERY_URI: &str = "http://0.0.0.0:50000";

const DEFAULT_FREQUENCY_MS: u64 = 10000; // 10 seconds

// Constants used for retry logic
const MAX_RETRIES: i32 = 10; // for demo purposes we will retry a maximum of 10 times
                             // By default we will wait 5 seconds between retry attempts
const DURATION_BETWEEN_ATTEMPTS: Duration = Duration::from_secs(5);

/// Get trailer weight's subscription information from managed subscribe endpoint.
///
/// # Arguments
/// * `managed_subscribe_uri` - The managed subscribe URI.
/// * `constraints` - Constraints for the managed topic.
async fn get_trailer_weight_subscription_info(
    managed_subscribe_uri: &str,
    constraints: Vec<Constraint>,
) -> Result<SubscriptionInfoResponse, Status> {
    // Create gRPC client.
    let mut client = ManagedSubscribeClient::connect(managed_subscribe_uri.to_string())
        .await
        .map_err(|err| Status::from_error(err.into()))?;

    let request = Request::new(SubscriptionInfoRequest {
        entity_id: trailer_v1::trailer::trailer_weight::ID.to_string(),
        constraints,
    });

    let response = client.get_subscription_info(request).await?;

    Ok(response.into_inner())
}

async fn get_trailer_temp_subscription_info(
    managed_subscribe_uri: &str,
    constraints: Vec<Constraint>,
) -> Result<SubscriptionInfoResponse, Status> {
    // Create gRPC client.
    let mut client = ManagedSubscribeClient::connect(managed_subscribe_uri.to_string())
        .await
        .map_err(|err| Status::from_error(err.into()))?;

    let request = Request::new(SubscriptionInfoRequest {
        entity_id: trailer_v1::trailer::trailer_temperature::ID.to_string(),
        constraints,
    });

    let response = client.get_subscription_info(request).await?;

    Ok(response.into_inner())
}

/// Receive Trailer Weight updates.
///
/// # Arguments
/// * `broker_uri` - The broker URI.
/// * `topic` - The topic.
async fn receive_trailer_info_updates(
    broker_uri: &str,
    topic: &str,
) -> Result<JoinHandle<()>, String> {
    // Create a unique id for the client.
    let client_id = format!("{MQTT_CLIENT_ID}-{}", Uuid::new_v4());

    let create_opts = mqtt::CreateOptionsBuilder::new()
        .server_uri(broker_uri)
        .client_id(client_id)
        .finalize();

    let client = mqtt::Client::new(create_opts)
        .map_err(|err| format!("Failed to create the client due to '{err:?}'"))?;

    let receiver = client.start_consuming();

    // Setup task to handle clean shutdown.
    let ctrlc_cli = client.clone();
    tokio::spawn(async move {
        _ = signal::ctrl_c().await;

        // Tells the client to shutdown consuming thread.
        ctrlc_cli.stop_consuming();
    });

    // Last Will and Testament
    let lwt = mqtt::MessageBuilder::new()
        .topic("test")
        .payload("Receiver lost connection")
        .finalize();

    let conn_opts = mqtt::ConnectOptionsBuilder::new_v5()
        .keep_alive_interval(Duration::from_secs(30))
        .clean_session(false)
        .will_message(lwt)
        .finalize();

    let _connect_response = client
        .connect(conn_opts)
        .map_err(|err| format!("Failed to connect due to '{err:?}"));

    let mut _subscribe_response = client
        .subscribe(topic, mqtt::types::QOS_1)
        .map_err(|err| format!("Failed to subscribe to topic {topic} due to '{err:?}'"));

    // Copy topic for separate thread.
    let topic_string = topic.to_string();

    let sub_handle = tokio::spawn(async move {
        for msg in receiver.iter() {
            if let Some(msg) = msg {
                // Here we log the message received. This could be expanded to parsing the message,
                // Obtaining the updates and making decisions based on them
                // For example, adjusting body functions or powertrain of the towing vehicle.
                info!("{}", msg);
            } else if !client.is_connected() {
                if client.reconnect().is_ok() {
                    _subscribe_response = client
                        .subscribe(topic_string.as_str(), mqtt::types::QOS_1)
                        .map_err(|err| {
                            format!("Failed to subscribe to topic {topic_string} due to '{err:?}'")
                        });
                } else {
                    break;
                }
            }
        }

        if client.is_connected() {
            debug!("Disconnecting");
            client.unsubscribe(topic_string.as_str()).unwrap();
            client.disconnect(None).unwrap();
        }
    });

    Ok(sub_handle)
}

async fn read_weight(provider_weight_endpoint_info: Option<EndpointInfo>, frequency_constraint: Constraint) -> Result<(), Box<dyn std::error::Error>> {
    if provider_weight_endpoint_info.is_some() {
        let managed_weight_subscribe_uri = provider_weight_endpoint_info.ok_or("Maximum amount of retries was reached while trying to retrieve the digital twin provider.")?.uri;
        info!("The Managed Subscribe URI for the TrailerWeight property's provider is {managed_weight_subscribe_uri}");    

        // Get the subscription information for a managed topic with constraints.
        let subscription_weight_info =
            get_trailer_weight_subscription_info(&managed_weight_subscribe_uri, vec![frequency_constraint.clone()])
                .await?;

        // Deconstruct subscription information.
        let broker_weight_uri = get_uri(&subscription_weight_info.uri)?;
        let topic_weight = subscription_weight_info.context;
        info!("The broker URI for the TrailerWeight property's provider is {broker_weight_uri}");

        // Subscribe to topic.
        let sub_handle_weight = receive_trailer_info_updates(&broker_weight_uri, &topic_weight)
            .await
            .map_err(|err| Status::internal(format!("{err:?}")))?;

        signal::ctrl_c().await?;

        info!("The Consumer has completed. Shutting down...");

        // Wait for subscriber task to cleanly shutdown.
        _ = sub_handle_weight.await;

    } else {
        info!("The Managed Subscribe URI for the TrailerWeight property's provider is not found");    
    }

    Ok(())
}

async fn read_temperature (provider_temp_endpoint_info: Option<EndpointInfo>, frequency_constraint: Constraint) -> Result<(),  Box<dyn std::error::Error>>{

    if provider_temp_endpoint_info.is_some() {
        let managed_temp_subscribe_uri = provider_temp_endpoint_info.ok_or("Maximum amount of retries was reached while trying to retrieve the digital twin provider.")?.uri;
        info!("The Managed Subscribe URI for the TrailerTemp property's provider is {managed_temp_subscribe_uri}");

        let subscription_temp_info =
            get_trailer_temp_subscription_info(&managed_temp_subscribe_uri, vec![frequency_constraint.clone()])
                .await?;
        
        // Deconstruct subscription information.
        let broker_temp_uri = get_uri(&subscription_temp_info.uri)?;

        let topic_temp = subscription_temp_info.context;
        info!("The broker URI for the TrailerTemp property's provider is {broker_temp_uri}");

        // Subscribe to topic.
        let sub_handle_temp = receive_trailer_info_updates(&broker_temp_uri, &topic_temp)
            .await
            .map_err(|err| Status::internal(format!("{err:?}")))?;

        signal::ctrl_c().await?;

        info!("The Consumer has completed. Shutting down...");
    
        // Wait for subscriber task to cleanly shutdown.
        _ = sub_handle_temp.await;
    
    } else {
        info!("The Managed Subscribe URI for the TrailerTemp property's provider is not found");    
    }

    Ok(())
}


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Setup logging.
    Builder::new()
        .filter(None, LevelFilter::Info)
        .target(Target::Stdout)
        .init();

    info!("The Smart Trailer Application has started.");

    // Get the In-vehicle Digital Twin Uri from the service discovery system
    // This could be enhanced to add retries for robustness
    let invehicle_digital_twin_uri = discover_service_using_chariott(
        CHARIOTT_SERVICE_DISCOVERY_URI,
        INVEHICLE_DIGITAL_TWIN_SERVICE_NAMESPACE,
        INVEHICLE_DIGITAL_TWIN_SERVICE_NAME,
        INVEHICLE_DIGITAL_TWIN_SERVICE_VERSION,
        INVEHICLE_DIGITAL_TWIN_SERVICE_COMMUNICATION_KIND,
        INVEHICLE_DIGITAL_TWIN_SERVICE_COMMUNICATION_REFERENCE,
    )
    .await?;

    // Get subscription constraints.
    let frequency_ms = env::args()
        .find_map(|arg| {
            if arg.contains(FREQUENCY_MS_FLAG) {
                return Some(arg.replace(FREQUENCY_MS_FLAG, ""));
            }

            None
        })
        .unwrap_or_else(|| DEFAULT_FREQUENCY_MS.to_string());

    // Retrieve the provider URI.
    let mut provider_weight_endpoint_info = None;
    let mut provider_temp_endpoint_info = None;
    let mut retries: i32 = 0;
    while provider_weight_endpoint_info.is_none() || provider_temp_endpoint_info.is_none() {
        provider_weight_endpoint_info = match discover_digital_twin_provider_using_ibeji(
            &invehicle_digital_twin_uri,
            trailer_v1::trailer::trailer_weight::ID,
            digital_twin_protocol::GRPC,
            &[digital_twin_operation::MANAGEDSUBSCRIBE.to_string()],
        )
        .await
        {
            Ok(response) => Some(response),
            Err(status) => {
                info!(
                    "A provider was not found in the digital twin service for id '{}' with: '{:?}'",
                    trailer_v1::trailer::trailer_weight::ID,
                    status
                );
                None
            }
        };

        provider_temp_endpoint_info = match discover_digital_twin_provider_using_ibeji(
            &invehicle_digital_twin_uri,
            trailer_v1::trailer::trailer_temperature::ID,
            digital_twin_protocol::GRPC,
            &[digital_twin_operation::MANAGEDSUBSCRIBE.to_string()],
        )
        .await
        {
            Ok(response) => Some(response),
            Err(status) => {
                info!(
                    "A provider was not found in the digital twin service for id '{}' with: '{:?}'",
                    trailer_v1::trailer::trailer_temperature::ID,
                    status
                );
                None
            }
        };

        if (provider_weight_endpoint_info.is_none() && provider_temp_endpoint_info.is_none()) && retries < MAX_RETRIES {
            info!("Retrying FindById to retrieve the properties provider endpoint in {DURATION_BETWEEN_ATTEMPTS:?}.");
            sleep(DURATION_BETWEEN_ATTEMPTS).await;
            retries += 1;
        } else {
            break;
        }
    }

    // Create constraint for the managed subscribe call.
    let frequency_constraint = Constraint {
        r#type: constraint_type::FREQUENCY_MS.to_string(),
        value: frequency_ms.to_string(),
    };

    let futures = vec![
        read_weight(provider_weight_endpoint_info, frequency_constraint.clone()).boxed(), 
        read_temperature(provider_temp_endpoint_info, frequency_constraint.clone()).boxed()];
    let _results = future::join_all(futures).await;

    Ok(())
}
