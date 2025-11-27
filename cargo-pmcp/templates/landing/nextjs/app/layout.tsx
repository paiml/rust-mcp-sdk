import type { Metadata } from 'next'
import './globals.css'

// Server name from environment variable (baked at build time)
const serverName = process.env.MCP_SERVER_NAME || 'MCP Server'

export const metadata: Metadata = {
  title: `${serverName} - MCP Server`,
  description: `Landing page for ${serverName} MCP server`,
}

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="en">
      <body className="antialiased">{children}</body>
    </html>
  )
}
