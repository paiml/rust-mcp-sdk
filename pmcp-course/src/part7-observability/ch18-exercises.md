# Chapter 18 Exercises

These exercises help you apply operations and monitoring patterns for production MCP servers.

## AI-Guided Exercises

The following exercises are designed for AI-guided learning. Use an AI assistant with the course MCP server to get personalized guidance, hints, and feedback.

1. **Performance Load Testing** ⭐⭐⭐ Advanced (60 min)
   - Run `cargo pmcp loadtest` against a local MCP server
   - Configure concurrency levels and duration parameters
   - Interpret the latency histogram and throughput results
   - Identify bottlenecks and apply optimization techniques from Ch 18-03

2. **Dashboard and Alerting Setup** ⭐⭐ Intermediate (45 min)
   - Configure pmcp.run dashboard for a deployed server
   - Set up alerting rules for error rate and latency thresholds
   - Create a runbook for common alert scenarios
   - Practice incident response using the observability tools from Ch 18-01 and Ch 18-02

## Prerequisites

Before starting these exercises, ensure you have:
- Completed the observability chapters (Ch 17-18)
- A running MCP server (local or deployed) to test against
- Familiarity with `cargo pmcp` CLI commands

## Next Steps

After completing these exercises, continue to [Chapter 19: Server Composition](../part8-advanced/ch19-composition.md).
