#[macro_use]
extern crate chan;

use std::error::Error;
use libp2p::tcp::GenTcpConfig;
use libp2p::{
    core::{
        either::EitherTransport, muxing::StreamMuxerBox, transport, transport::upgrade::Version,
    },
    gossipsub::{self, Gossipsub, GossipsubConfigBuilder, GossipsubEvent, MessageAuthenticity},
    identify::{Identify, IdentifyConfig, IdentifyEvent},
    identity,
    multiaddr::Protocol,
    noise,
    ping::{self, PingEvent},
    pnet::{PnetConfig, PreSharedKey},
    swarm::SwarmEvent,
    tcp::TcpTransport,
    yamux::YamuxConfig,
    Multiaddr, NetworkBehaviour, PeerId, Swarm, Transport,
};
use rand::Rng;
use std::time::Duration;
use crate::transport::Boxed;
use futures::prelude::*;

fn get_psk() -> PreSharedKey {
    let key = rand::thread_rng().gen::<[u8; 32]>();
    PreSharedKey::new(key)
}

pub fn build_transport(
    keypair: identity::Keypair,
    psk: PreSharedKey,
) -> Boxed<(PeerId, StreamMuxerBox)> {
    let noise_config = noise::NoiseAuthenticated::xx(&keypair).unwrap();

    TcpTransport::new(GenTcpConfig::default().nodelay(true))
        .and_then(move |socket,_| PnetConfig::new(psk).handshake(socket))
        .upgrade(Version::V1)
        .authenticate(noise_config)
        .multiplex(YamuxConfig::default())
        .timeout(Duration::from_secs(20))
        .boxed()
}

#[async_std::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let tick = chan::tick_ms(5000);

    let local_key = identity::Keypair::generate_ed25519();
    let local_peer_id = PeerId::from(local_key.public());

    loop {
        chan_select! {
            tick.recv() => {
                let psk = get_psk();
                println!("using pre shared key with fingerprint: {}", psk.fingerprint());

                let transport = build_transport(local_key.clone(), psk);
                let behaviour = ping::Behaviour::new(ping::Config::new().with_keep_alive(true));

                let mut swarm = Swarm::new(transport, behaviour, local_peer_id);
                swarm.listen_on("/ip4/0.0.0.0/tcp/61000".parse()?)?;

                if let Some(addr) = std::env::args().nth(1) {
                        let remote: Multiaddr = addr.parse()?;
                        swarm.dial(remote)?;
                        println!("Dialed {}", addr)
                };
                loop {
                    match swarm.select_next_some().await {
                        SwarmEvent::NewListenAddr { address, .. } => println!("Listening on {:?}", address),
                        SwarmEvent::Behaviour(event) => println!("{:?}", event),
                        _ => println!("nothing")
                    };
                }
            },
        }
    }
}
