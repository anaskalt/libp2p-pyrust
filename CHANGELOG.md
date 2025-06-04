# Changelog

## 0.2.0 (2025-06-05)

### 🚀 Major Enhancements
- **Enhanced CI/CD Pipeline**: Comprehensive multi-platform wheel generation
  - Complete Python version support: 3.8, 3.9, 3.10, 3.11, 3.12
  - Cross-platform architecture coverage: Linux (x86_64, aarch64, i686, armv7, ppc64le, s390x), macOS (Intel + Apple Silicon), Windows (x86_64, i686, aarch64)
  - Automated public release generation from private repository
  - Production-optimized build configurations

### 🔧 Technical Improvements
- **Performance Optimizations**: Enhanced Rust compilation settings for minimal binary size
  - Link-time optimization (LTO) enabled for maximum performance
  - Debug symbol stripping for reduced package size
  - Optimized panic handling for production environments

### 📦 Distribution Enhancements
- **Private Repository with Public Releases**: Source code remains private while enabling public wheel distribution
- **Comprehensive Platform Support**: Enhanced maturin configuration for cross-platform compatibility
- **Enhanced Metadata**: Improved package classifiers and platform declarations

### 🐛 Bug Fixes
- **Windows ARM64 Support**: Fixed Python version consistency across all Windows architectures
- **Artifact Collection**: Resolved CI/CD artifact handling for reliable release generation

### 📚 Documentation
- **Installation Guide**: Comprehensive platform-specific installation instructions
- **Usage Examples**: Enhanced quick start examples with error handling
- **Compatibility Matrix**: Detailed platform and Python version support information

## 0.1.0 (2024-06-23)
- Initial release of `libp2p_pyrust`.
- Basic libp2p functionality implemented.
- Python bindings for core functionality.
