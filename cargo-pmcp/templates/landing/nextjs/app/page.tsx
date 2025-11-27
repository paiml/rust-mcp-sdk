import Hero from './components/Hero'
import Features from './components/Features'
import Installation from './components/Installation'

export default function Home() {
  return (
    <main className="min-h-screen bg-gradient-to-b from-gray-50 to-white">
      <Hero />
      <Features />
      <Installation />

      {/* Footer */}
      <footer className="py-8 text-center text-gray-500 text-sm border-t">
        <p>
          Powered by{' '}
          <a
            href="https://pmcp.run"
            className="text-blue-600 hover:underline"
            target="_blank"
            rel="noopener noreferrer"
          >
            pmcp.run
          </a>
        </p>
      </footer>
    </main>
  )
}
