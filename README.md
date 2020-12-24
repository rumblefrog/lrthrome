# Lrthrome

Fast TCP-server based IPv4 CIDR filter lookup server over minimal binary protocol, and memory footprint.

Lrthrome is:

- A stateless TCP server and protocol with optional identification.
- In-memory IPv4 CIDR lookup tree.
- Interval lookup tree automatic updating.
- Limited peer keep-alive duration.
- Request GCRA based ratelimiter.

## Current implemented sources

|  Name  |   Field   |       Description        |
| ------ | --------- | ------------------------ |
| Remote | `remotes` | HTTP request to endpoint |
