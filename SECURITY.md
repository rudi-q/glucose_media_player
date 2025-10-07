# Security Policy

## Supported Versions

We take security seriously and aim to keep glucose safe for all users. Currently, we support security updates for the following versions:

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| < 0.2.0 | :x:                |

**Note:** We recommend always using the latest version to ensure you have the most recent security patches and improvements.

## Reporting a Vulnerability

If you discover a security vulnerability in glucose, please help us address it responsibly.

### How to Report

**Please DO NOT report security vulnerabilities through public GitHub issues.**

Instead, please report them via one of these methods:

1. **GitHub Security Advisories** (Preferred)
   - Go to the [Security tab](https://github.com/rudi-q/glucose_media_player/security/advisories)
   - Click "Report a vulnerability"
   - Fill out the form with details

2. **Email**
   - Send an email to: [reach@rudi.engineer](mailto:reach@rudi.engineer)
   - If this email is not available, create a private GitHub issue and tag the maintainers

### What to Include

When reporting a vulnerability, please include:

- **Description**: Clear description of the vulnerability
- **Impact**: What could an attacker accomplish with this vulnerability?
- **Steps to Reproduce**: Detailed steps to reproduce the issue
- **Affected Versions**: Which versions of glucose are affected?
- **Proof of Concept**: Code, screenshots, or videos demonstrating the issue (if applicable)
- **Suggested Fix**: If you have ideas for fixing the issue (optional)
- **Your Environment**: OS, glucose version, and relevant system details

### Response Timeline

- **Initial Response**: Within 48 hours of report
- **Status Update**: Within 7 days with preliminary assessment
- **Fix Timeline**: Varies based on severity
  - **Critical**: 1-7 days
  - **High**: 7-30 days
  - **Medium**: 30-90 days
  - **Low**: Best effort

### What to Expect

1. **Acknowledgment**: We'll confirm receipt of your report
2. **Investigation**: We'll investigate and validate the vulnerability
3. **Fix Development**: We'll work on a fix (may involve you if needed)
4. **Disclosure**: We'll coordinate disclosure timing with you
5. **Credit**: We'll credit you in the security advisory (unless you prefer anonymity)

## Security Considerations

### Known Security Scope

glucose is a desktop video player that:
- Reads local video files
- Loads subtitle files (SRT, VTT, ASS, SSA, SUB)
  - SRT files are automatically converted to WebVTT format
  - VTT files are used directly without conversion
  - ASS, SSA, and SUB formats are detected but not yet supported (planned for future releases)
- Generates video thumbnails
- Accesses local file system

### Security Best Practices for Users

1. **Download from Official Sources**
   - Only download glucose from official GitHub releases
   - Verify checksums/signatures when available

2. **Keep Updated**
   - Regularly update to the latest version
   - Enable auto-updates if available

3. **File Sources**
   - Only open video files from trusted sources
   - Be cautious with files from unknown origins

4. **Permissions**
   - glucose only needs file system read access
   - Review requested permissions during installation

### Areas of Concern

While glucose is designed with security in mind, be aware of:

1. **File Parsing**
   - Video codecs and container formats may have vulnerabilities
   - We rely on system codecs and WebKit for video decoding

2. **Subtitle Files**
   - SRT files are parsed and automatically converted to WebVTT
   - VTT files are loaded directly
   - ASS, SSA, and SUB formats are currently rejected (not yet supported)
   - Malformed subtitle files could potentially cause issues

3. **Thumbnail Generation**
   - Canvas-based thumbnail generation accesses video frames
   - Cross-origin restrictions help protect against tainted canvases

4. **File System Access**
   - glucose scans directories for recent videos
   - File system access is read-only by default

## Security Updates

Security updates will be:
- Released as soon as possible after verification
- Announced in GitHub Security Advisories
- Documented in CHANGELOG.md
- Tagged with version bumps following semantic versioning

### Notification Channels

Stay informed about security updates:
- Watch the GitHub repository for security advisories
- Check release notes regularly
- Subscribe to GitHub notifications

## Disclosure Policy

We follow responsible disclosure:

1. **Private Disclosure**: Security issues are handled privately initially
2. **Fix Development**: We develop and test fixes before public disclosure
3. **Coordinated Release**: We coordinate release timing with reporters
4. **Public Disclosure**: After a fix is released, we publish security advisories
5. **CVE Assignment**: For significant vulnerabilities, we request CVE IDs

### Timeline

- **Day 0**: Vulnerability reported privately
- **Day 0-7**: Investigation and validation
- **Day 7-30**: Fix development and testing (varies by severity)
- **Day 30+**: Coordinated public disclosure with fix release

## Bug Bounty

Currently, we do not have a formal bug bounty program. However:
- We deeply appreciate security research
- Researchers will be credited in advisories
- We may provide swag or recognition for significant findings

## Third-Party Dependencies

glucose uses several third-party libraries:

### Frontend

- Svelte/SvelteKit
- Tauri API
- Vite

### Backend (Rust)

- Tauri framework
- tokio
- serde

We monitor these dependencies for security issues and update them regularly.

### Reporting Dependency Issues

If you find a vulnerability in a dependency:
1. Check if the upstream project is aware
2. Report to the upstream project first
3. Notify us if it affects glucose users

## Security Hardening

We implement security best practices:

### Code Level

- Input validation and sanitization
- Safe Rust practices (avoiding unsafe code where possible)
- Regular dependency updates
- Code review for security implications

### Build & Distribution

- CI/CD pipeline security
- Code signing (when available)
- Checksum verification
- Supply chain security

### Runtime

- Principle of least privilege
- Sandboxing where possible (via Tauri)
- Safe file handling
- Memory safety (Rust benefits)

## Questions?

If you have questions about glucose's security that don't involve a specific vulnerability:
- Open a GitHub Discussion
- Create a non-security GitHub issue
- Check existing documentation

Thank you for helping keep glucose and its users safe! 🔒
