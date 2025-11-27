// Environment variables are baked at build time
const serverName = process.env.MCP_SERVER_NAME || 'MCP Server'
const endpoint = process.env.MCP_ENDPOINT || ''

export default function Hero() {
  return (
    <div className="container mx-auto px-4 py-20">
      <div className="text-center max-w-3xl mx-auto">
        <h1 className="text-5xl font-bold text-gray-900 mb-6">
          {serverName}
        </h1>
        <p className="text-xl text-gray-600 mb-8">
          A powerful MCP server for AI assistants
        </p>

        {endpoint && (
          <div className="bg-gray-100 rounded-lg p-4 inline-block">
            <p className="text-sm text-gray-500 mb-1">Endpoint</p>
            <code className="text-blue-600 font-mono">{endpoint}</code>
          </div>
        )}

        <div className="mt-8">
          <a
            href="#installation"
            className="bg-blue-600 text-white px-8 py-3 rounded-lg font-semibold hover:bg-blue-700 transition-colors"
          >
            Get Started
          </a>
        </div>
      </div>
    </div>
  )
}
