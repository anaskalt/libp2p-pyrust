pub mod crypto;
/// The `libp2p_pyrust` crate provides Python bindings for libp2p implemented in Rust.
pub mod network;

use network::P2PNetwork;

use lazy_static::lazy_static;
use pyo3::prelude::*;
use pyo3::types::PyBytes;
use std::sync::Arc;
use tokio::sync::Mutex as AsyncMutex;

lazy_static! {
    static ref GLOBAL_P2P_NETWORK: Arc<AsyncMutex<Option<P2PNetwork>>> = Arc::new(AsyncMutex::new(None));
}

/// Initializes the global P2P network.
///
/// # Arguments
///
/// * `py` - The Python interpreter state.
/// * `bootnodes` - A list of bootnodes to connect to.
/// * `port` - The port to bind to.
/// * `key_path` - The path to the key file.
/// * `topic_name` - The topic name to subscribe to.
///
/// # Returns
///
/// A Python `None` object on success.
#[pyfunction]
fn init_global_p2p_network(
    py: Python<'_>,
    bootnodes: Vec<String>,
    port: u64,
    key_path: String,
    topic_name: String,
) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let p2p_net = P2PNetwork::new(bootnodes, port, key_path, topic_name)
            .await
            .expect("start p2p server error");
        {
            let mut network = tp.lock().await;
            *network = Some(p2p_net);
        }
        Ok(Python::with_gil(|py| py.None()))
    })
}

/// Retrieves the list of connected peers in the global P2P network.
///
/// # Arguments
///
/// * `py` - The Python interpreter state.
///
/// # Returns
///
/// A list of connected peer IDs.
#[pyfunction]
fn get_global_connected_peers(py: Python<'_>) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;
        if let Some(network) = &*network {
            let peers = network
                .get_connected_peers()
                .await
                .expect("Failed to get connected peers");
            Python::with_gil(|py| {
                Ok(peers
                    .into_iter()
                    .map(|peer| peer.to_string())
                    .collect::<Vec<String>>()
                    .into_py(py))
            })
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "P2PNetwork is not initialized",
            ))
        }
    })
}

/// Connects to a peer in the global P2P network.
///
/// # Arguments
///
/// * `py` - The Python interpreter state.
/// * `addr` - The address of the peer to connect to.
///
/// # Returns
///
/// A Python `None` object on success.
#[pyfunction]
fn connect_to_peer(py: Python<'_>, addr: String) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;
        if let Some(network) = &*network {
            network.connect_to_peer(&addr).await.map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Failed to connect to peer: {}", e))
            })?;
            Ok(Python::with_gil(|py| py.None()))
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "P2PNetwork is not initialized",
            ))
        }
    })
}

/// Publishes a message to the global P2P network.
///
/// # Arguments
///
/// * `py` - The Python interpreter state.
/// * `message` - The message to publish.
///
/// # Returns
///
/// A Python `None` object on success.
#[pyfunction]
fn publish_message(py: Python<'_>, message: Vec<u8>) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;
        if let Some(network) = &*network {
            network.publish_message(message).await.map_err(|e| {
                PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Failed to publish message: {}", e))
            })?;
            Ok(Python::with_gil(|py| py.None()))
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "P2PNetwork is not initialized",
            ))
        }
    })
}

/// Subscribes to messages in the global P2P network.
///
/// # Arguments
///
/// * `py` - The Python interpreter state.
/// * `callback` - The Python callback function to call with each message.
///
/// # Returns
///
/// A Python `None` object on success.
#[pyfunction]
fn subscribe_to_messages(py: Python, callback: PyObject) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let mut receiver_result = {
            let network = tp.lock().await;
            network
                .as_ref()
                .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyRuntimeError, _>("P2PNetwork is not initialized"))?
                .subscribe_to_messages()
        };

        while let Ok(message_event) = receiver_result.recv().await {
            Python::with_gil(|py| {
                let py_message = PyBytes::new(py, &message_event.content);
                callback.call1(py, (py_message,)).unwrap();
            });
        }
        Ok(Python::with_gil(|py| py.None()))
    })
}

/// Retrieves the host addresses of the global P2P network.
///
/// # Arguments
///
/// * `py` - The Python interpreter state.
///
/// # Returns
///
/// A list of host addresses.
#[pyfunction]
fn get_host_addrs(py: Python<'_>) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;
        if let Some(network) = &*network {
            let addrs = network.full_addrs.read().await;
            Python::with_gil(|py| {
                Ok(addrs
                    .clone()
                    .into_iter()
                    .map(|peer| peer.to_string())
                    .collect::<Vec<String>>()
                    .into_py(py))
            })
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "P2PNetwork is not initialized",
            ))
        }
    })
}

/// Retrieves the peer ID of the global P2P network.
///
/// # Arguments
///
/// * `py` - The Python interpreter state.
///
/// # Returns
///
/// The peer ID as a string.
#[pyfunction]
fn get_peer_id(py: Python<'_>) -> PyResult<&PyAny> {
    let tp = GLOBAL_P2P_NETWORK.clone();
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let network = tp.lock().await;
        if let Some(network) = &*network {
            let peer_id = network.peer_id.to_string();
            Python::with_gil(|_py| Ok(peer_id))
        } else {
            Err(PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(
                "P2PNetwork is not initialized",
            ))
        }
    })
}

/// Generates an Ed25519 key pair and saves it to a file.
///
/// # Arguments
///
/// * `path` - The path to save the key pair to.
///
/// # Returns
///
/// The path where the key pair was saved.
#[pyfunction]
fn generate_ed25519_keypair(path: String) -> PyResult<String> {
    let keypair = crypto::generate_ed25519_keypair();
    crypto::save_keypair_to_file(&keypair, path.clone())?;
    Ok(path)
}

/// Signs data using a key pair from a file.
///
/// # Arguments
///
/// * `file_path_str` - The path to the key pair file.
/// * `data` - The data to sign.
///
/// # Returns
///
/// The signature as a base64 encoded string.
#[pyfunction]
fn sign_data(file_path_str: String, data: Vec<u8>) -> PyResult<String> {
    let keypair = crypto::sign_data_with_key(file_path_str, data)?;
    Ok(keypair)
}

/// Retrieves and serializes the public key from a key pair file.
///
/// # Arguments
///
/// * `path` - The path to the key pair file.
///
/// # Returns
///
/// The serialized public key as a base64 encoded string.
#[pyfunction]
fn get_serialize_public_key(path: String) -> PyResult<String> {
    let pub_key = crypto::serialize_public_key_to_string(path)?;
    Ok(pub_key)
}

pub fn get_version() -> String {
    let version = env!("CARGO_PKG_VERSION").to_string();
    // cargo uses "1.0-alpha1" etc. while python uses "1.0.0a1", this is not full compatibility,
    // but it's good enough for now
    // see https://docs.rs/semver/1.0.9/semver/struct.Version.html#method.parse for rust spec
    // see https://peps.python.org/pep-0440/ for python spec
    // it seems the dot after "alpha/beta" e.g. "-alpha.1" is not necessary, hence why this works
    version.replace("-alpha", "a").replace("-beta", "b")
}

/// The main Python module definition.
#[pymodule]
fn libp2p_pyrust(_py: Python, m: &PyModule) -> PyResult<()> {
    let version = get_version();
    m.add("__version__", version.clone())?;
    // keep VERSION for compatibility
    m.add("VERSION", version)?;
    m.add_wrapped(wrap_pyfunction!(init_global_p2p_network))?;
    m.add_wrapped(wrap_pyfunction!(get_global_connected_peers))?;
    m.add_wrapped(wrap_pyfunction!(connect_to_peer))?;
    m.add_wrapped(wrap_pyfunction!(publish_message))?;
    m.add_wrapped(wrap_pyfunction!(subscribe_to_messages))?;
    m.add_wrapped(wrap_pyfunction!(generate_ed25519_keypair))?;
    m.add_wrapped(wrap_pyfunction!(get_host_addrs))?;
    m.add_wrapped(wrap_pyfunction!(get_peer_id))?;
    m.add_wrapped(wrap_pyfunction!(sign_data))?;
    m.add_wrapped(wrap_pyfunction!(get_serialize_public_key))?;
    Ok(())
}
