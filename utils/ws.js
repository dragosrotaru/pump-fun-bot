import WebSocket from "ws";

const ws = new WebSocket(process.env.WSS_ENDPOINT || "wss://api.mainnet-beta.solana.com");

ws.on("open", () => {
    console.log("Connected to Solana WebSocket");

    const subscriptionMessage = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "accountSubscribe",
        "params": [
          "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P",
          {
            "encoding": "jsonParsed",
            "commitment": "finalized"
          }
        ]
      }

    ws.send(JSON.stringify(subscriptionMessage));
});

ws.on("message", (data) => {
    console.log("Received:", data.toString());
});

ws.on("close", () => {
    console.log("WebSocket closed");
});

ws.on("error", (error) => {
    console.error("WebSocket error:", error);
});

// Keep the connection alive
setInterval(() => ws.ping(), 5000);
