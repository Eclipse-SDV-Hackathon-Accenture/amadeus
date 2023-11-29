import digital_twin_get_provider_pb2 as dt

def main():
    request_msg = dt.GetRequest()
    request_msg.entity_id = "1111"
    response_msg = dt.GetResponse()
    response_msg.property_value = 12
<<<<<<< HEAD
=======
    dt
>>>>>>> 09fd7f1 (added python sample)
    print(request_msg)
    print(response_msg)
    
    
if __name__ == "__main__":
    exit(main())