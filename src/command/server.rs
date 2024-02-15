// use tokio::{net::UnixListener, runtime::Builder};
// use tokio_stream::wrappers::UnixListenerStream;
//
// use super::SOCKET_PATH;
//
// pub fn run() {
//     let server = UnixListener::bind(SOCKET_PATH).unwrap();
//     let _stream = UnixListenerStream::new(server);
//
//     let _runtime = Builder::new_multi_thread()
//         .worker_threads(2)
//         .thread_name("scape-command-server-worker")
//         .build()
//         .unwrap();
//
//     // runtime.spawn({
//     //     let x = server.accept();
//     // });
//     //     server
//     //         .incoming()
//     //         .into_future()
//     //         .map_err(|(e, _)| e)
//     //         .and_then(|(sock, _)| io::read_to_end(sock.unwrap(), Vec::new()))
//     //         .map(|bytes| println!("{}", serde_json::from_slice::<String>(&bytes.1).unwrap()))
//     //         .map_err(|e| panic!("{:?}", e)),
//     // );
// }
