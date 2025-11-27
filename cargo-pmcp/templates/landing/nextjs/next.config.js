/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'export',           // REQUIRED: Produces static HTML in out/ directory
  trailingSlash: true,        // REQUIRED: Better URL handling for static hosting
  images: {
    unoptimized: true,        // REQUIRED: Static export doesn't support image optimization
  },
  // Environment variables baked at build time
  env: {
    MCP_SERVER_NAME: process.env.MCP_SERVER_NAME || '',
    MCP_ENDPOINT: process.env.MCP_ENDPOINT || '',
  },
}

module.exports = nextConfig
