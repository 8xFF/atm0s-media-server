import { WHIPClient } from "./whip.js"

window.start = async () => {
    console.log("Will start");
    if (window.whip_instance) {
        window.whip_instance.stop();
    }

    if (window.stream_instance) {
        window.stream_instance.getTracks().forEach(track => track.stop());
    }

    //Get mic+cam
    const stream = await navigator.mediaDevices.getUserMedia({audio:true, video:true});

    document.getElementById("video").srcObject = stream;

    //Create peerconnection
    const pc = new RTCPeerConnection();

    //Send all tracks
    for (const track of stream.getTracks()) {
        //You could add simulcast too here
        pc.addTransceiver(track, {
            direction: "sendonly",
            streams:  [stream],
            // sendEncodings: [
            //     { rid: "0", active: true, scaleResolutionDownBy: 2},
            //     { rid: "1", active: true, scaleResolutionDownBy: 2},
            //     { rid: "2", active: true },
            // ],
        });
    }

    //Create whip client
    const whip = new WHIPClient();

    const url = "/whip/endpoint";
    const token = document.getElementById("room-id").value;

    //Start publishing
    whip.publish(pc, url, token);

    window.whip_instance = whip;
    window.stream_instance = stream;
}

window.stop = async () => {
    if (window.whip_instance) {
        window.whip_instance.stop();
    }

    if (window.stream_instance) {
        window.stream_instance.getTracks().forEach(track => track.stop());
    }

    document.getElementById("video").srcObject = null;
}