const ws = new WebSocket("ws://localhost:8050/ws/cctxs");
let connected = false;
ws.onopen = () => {
    connected = true;
    console.log("Connected to server");
};
ws.onmessage = (e) => console.log("Message:", e.data);
ws.onclose = () => {
    connected = false;
    console.log("Disconnected from server");
};
// setInterval(() => {
//     if (connected) {
//         ws.send("ping");
//     }
// }, 1000);