from setuptools import setup
from setuptools_rust import Binding, RustExtension

setup(
    name="libp2p_pyrust",
    version="0.1.0",
    author="Anastasios Kaltakis",
    author_email="anastasioskaltakis@gmail.com",
    description="A libp2p implementation in Rust for use with Python",
    long_description=open('README.rst').read(),
    long_description_content_type='text/x-rst',
    url="https://github.com/anaskalt/libp2p_pyrust",
    packages=["libp2p_pyrust"],
    rust_extensions=[RustExtension("libp2p_pyrust.libp2p_pyrust", path="Cargo.toml", binding=Binding.PyO3)],
    include_package_data=True,
    zip_safe=False,
    classifiers=[
        "Programming Language :: Python :: 3",
        "Programming Language :: Rust",
        "License :: OSI Approved :: MIT License",
        "Operating System :: OS Independent",
    ],
    python_requires='>=3.8',
    install_requires=[
        "setuptools-rust>=0.12.1",
    ],
)
