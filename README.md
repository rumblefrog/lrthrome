# Lrthrome

Fast and light TCP-server based IPv4 CIDR filter lookup server over minimal binary protocol, and memory footprint.

- Fast, with target support up to 25 million worst case lookups per second.
- Light, full IPv4 BGP table of more than 600,000 entries fits in less than 5 MB memory space.
- Automatic, with customizable interval updates to in-memory lookup table, requiring zero maintenance for Lrthrome.

## Current implemented sources

|  Name  |   Field   |       Description        |
| ------ | --------- | ------------------------ |
| Remote | `remotes` | HTTP request to endpoint |
