const net = require('net');

/**
 * Basic helper to send a command and wait for a single RESP-style response.
 * Note: This server expects raw strings followed by a newline, 
 * rather than full RESP-encoded commands.
 */
function sendCommand(client, cmd) {
    return new Promise((resolve, reject) => {
        client.write(`${cmd}\n`, (err) => {
            if (err) return reject(err);
        });

        client.once('data', (data) => {
            const response = data.toString();
            // Simple cleaning of common RESP responses for the example output
            let cleanRes = response.trim();
            if (response.startsWith('+') || response.startsWith(':')) {
                cleanRes = response.slice(1).trim();
            } else if (response.startsWith('$')) {
                const lines = response.split('\r\n');
                cleanRes = lines.length >= 2 ? lines[1] : response.trim();
            }
            resolve(cleanRes);
        });
    });
}

async function main() {
    console.log('Connecting to Redix at 127.0.0.1:6380...');

    const client = net.createConnection({ port: 6380, host: '127.0.0.1' }, async () => {
        console.log('Connected!\n');

        try {
            // 1. SET key
            console.log('> SET example_key Hello-NodeJS');
            let res = await sendCommand(client, 'SET example_key Hello-NodeJS');
            console.log('Response:', res, '\n');

            // 2. GET key
            console.log('> GET example_key');
            res = await sendCommand(client, 'GET example_key');
            console.log('Response:', res, '\n');

            // 3. DEL key
            console.log('> DEL example_key');
            res = await sendCommand(client, 'DEL example_key');
            console.log('Response:', res, '\n');

            // 4. GET key again
            console.log('> GET example_key');
            res = await sendCommand(client, 'GET example_key');
            console.log('Response:', res, '\n');

            console.log('Example complete.');
        } catch (err) {
            console.error('Error during operations:', err);
        } finally {
            client.end();
        }
    });

    client.on('error', (err) => {
        console.error('Connection Error:', err.message);
        process.exit(1);
    });
}

main().catch(console.error);
