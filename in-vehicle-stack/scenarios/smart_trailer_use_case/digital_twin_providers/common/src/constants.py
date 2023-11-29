# Digital Twin Operations (python)
# Converted by ChatGPT 4.0 from a RUST file
# Comment by ChatGPT
# In this Python code:
# Each Rust mod is translated into a Python class.
# The Rust string constants (pub const) are translated into class variables in Python.
# The structure and naming convention are preserved to maintain readability and consistency with the original Rust code.
#
class DigitalTwinOperation:
    GET = "Get"
    SET = "Set"
    SUBSCRIBE = "Subscribe"
    UNSUBSCRIBE = "Unsubscribe"
    INVOKE = "Invoke"
    STREAM = "Stream"
    MANAGEDSUBSCRIBE = "ManagedSubscribe"

# Digital Twin Protocols
class DigitalTwinProtocol:
    GRPC = "grpc"
    MQTT = "mqtt"

# Chariott Constants
class Chariott:
    INVEHICLE_DIGITAL_TWIN_SERVICE_NAMESPACE = "sdv.ibeji"
    INVEHICLE_DIGITAL_TWIN_SERVICE_NAME = "invehicle_digital_twin"
    INVEHICLE_DIGITAL_TWIN_SERVICE_VERSION = "1.0"
    INVEHICLE_DIGITAL_TWIN_SERVICE_COMMUNICATION_KIND = "grpc+proto"
    INVEHICLE_DIGITAL_TWIN_SERVICE_COMMUNICATION_REFERENCE = "https://github.com/eclipse-ibeji/ibeji/blob/main/interfaces/digital_twin/v1/digital_twin.proto"

# Constraint Types for Subscribe Requests
class ConstraintType:
    FREQUENCY_MS = "frequency_ms"
