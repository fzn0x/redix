# Node.js Example

This example demonstrates how to interact with `Redix` using the standard `redis` Node.js package.

## Prerequisites

- [Node.js](https://nodejs.org/) (v14+)
- A running `Redix` server (e.g., on `127.0.0.1:6380`)

## Running the Example

1. Navigate to this directory:
   ```bash
   cd examples/nodejs
   ```

2. (Optional) Install dependencies:
   ```bash
   npm install
   ```

3. Run the script:
   ```bash
   npm start
   ```

## Code Preview

The script uses Node's `net` module to send raw commands:

```javascript
const net = require('net');
const client = net.createConnection({ port: 6380 });
client.write('SET key value\n');
```
