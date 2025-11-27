const features = [
  {
    title: 'Tools',
    description: 'Powerful tools for AI assistants to interact with your service',
    icon: '🛠️',
  },
  {
    title: 'Resources',
    description: 'Access data and content through standardized resource URIs',
    icon: '📦',
  },
  {
    title: 'Prompts',
    description: 'Pre-built prompts for common use cases',
    icon: '💬',
  },
]

export default function Features() {
  return (
    <div className="container mx-auto px-4 py-20 bg-white">
      <h2 className="text-3xl font-bold text-center mb-12">Capabilities</h2>

      <div className="grid md:grid-cols-3 gap-8 max-w-5xl mx-auto">
        {features.map((feature) => (
          <div
            key={feature.title}
            className="p-6 rounded-xl border border-gray-200 hover:shadow-lg transition-shadow"
          >
            <div className="text-4xl mb-4">{feature.icon}</div>
            <h3 className="text-xl font-semibold mb-2">{feature.title}</h3>
            <p className="text-gray-600">{feature.description}</p>
          </div>
        ))}
      </div>
    </div>
  )
}
