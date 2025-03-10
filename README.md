<div align="center">

<img src="logo.png" width="25%" />

# CharizHard OTA API

A cutting-edge hardware solution designed to secure data exchanges and protect devices from hardware-based threats. This is the server-side implementation for over-the-air updates.
</div>

##  API Endpoints

### 1. **Welcome Message**
- **Endpoint**: `/`
- **Method**: `GET`
- **Description**: A friendly welcome message for the API.
- **Response**:
  ```
  Welcome to Charizhard OTA! Check /latest/ to get the latest firmware.
  ```

---

### 2. **Get Latest Firmware**
- **Endpoint**: `/latest/`
- **Method**: `GET`
- **Description**: Fetches the latest firmware based on semantic versioning (major and minor version numbers).
- **Response**:
  - **200 OK**: Returns the latest firmware binary for download.
  - **404 Not Found**: If no firmware is found.
  - **500 Internal Server Error**: If there's an issue reading the firmware directory.
- **Headers**:
  - `Content-Disposition`: `attachment; filename=<filename>`
  - `Content-Type`: `application/octet-stream`

---

### 3. **Upload New Firmware**
- **Endpoint**: `/firmware/:file`
- **Method**: `PUT`
- **Description**: Uploads a new firmware binary to the server.
- **Path Parameter**:
  - `file`: The name of the firmware file (e.g., `charizhard.V1.0.bin`).
- **Response**:
  - **200 OK**: JSON response indicating the number of bytes written.
    ```json
    { "bytes": 12345 }
    ```
  - **500 Internal Server Error**: If there's an issue writing the file.

---

### 4. **Download Specific Firmware**
- **Endpoint**: `/firmware/:file`
- **Method**: `GET`
- **Description**: Downloads a specific firmware file by name.
- **Path Parameter**:
  - `file`: The name of the firmware file (e.g., `charizhard.V1.0.bin`).
- **Response**:
  - **200 OK**: Returns the requested firmware binary for download.
  - **404 Not Found**: If the requested firmware file is not found.
- **Headers**:
  - `Content-Disposition`: `attachment; filename=<filename>`
  - `Content-Type`: `application/octet-stream`

---

### 5. **Delete Firmware**
- **Endpoint**: `/firmware/:file`
- **Method**: `DELETE`
- **Description**: Deletes a specific firmware file by name.
- **Path Parameter**:
  - `file`: The name of the firmware file (e.g., `charizhard.V1.0.bin`).
- **Response**:
  - **200 OK**: If the file was successfully deleted.
  - **404 Not Found**: If the file could not be found or deleted.

---

## 🛠️ Setup Instructions

1. **Clone the Repository**:
   ```fish
   git clone https://github.com/your-repo/charizhard-ota.git
   cd charizhard-ota
   ```

2. **Run the Application**:
   Ensure you have Rust installed. Then, run:
   ```fish
   cargo run
   ```

3. **Access the API**:
   Open your browser or use a tool like `curl` or Postman to access the API at `http://127.0.0.1:8080`.

---

## Directory Structure
- **`/bin/`**: Stores all firmware binaries.

---

##  Notes
- Ensure the `/bin/` directory exists in the project root.
- Firmware files must follow the naming convention: `name.V<major>.<minor>.bin` (e.g., `charizhard.V1.0.bin`).

---

##  Example Usage

### Upload Firmware
```fish
curl -X PUT --data-binary @charizhard.V1.0.bin http://127.0.0.1:8080/firmware/charizhard.V1.0.bin
```

### Get Latest Firmware
```fish
curl -X GET http://127.0.0.1:8080/latest/ --output latest_firmware.bin
```

### Download Specific Firmware
```fish
curl -X GET http://127.0.0.1:8080/firmware/charizhard.V1.0.bin --output charizhard.V1.0.bin
```

### Delete Firmware
```fish
curl -X DELETE http://127.0.0.1:8080/firmware/charizhard.V1.0.bin
```

---

##  Security Considerations
- Validate user inputs to prevent directory traversal attacks.
- Add authentication for sensitive operations (e.g., uploading or deleting firmware).

---

Happy coding! 🎉