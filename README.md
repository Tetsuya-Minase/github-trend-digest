# GitHub Trending Summary

## 1. Project Overview

This project is a Rust application that daily fetches trending repository information from GitHub, summarizes it using the Gemini AI, and notifies a Discord channel. It serves as a tool to efficiently keep up with the latest trends in the developer community.

## 2. Architecture

This project adopts the **Hexagonal Architecture** (also known as Ports and Adapters).

### Why Hexagonal Architecture?
- **Separation of Concerns**: The core business logic (domain) is completely isolated from external infrastructure details (like web APIs, databases, or notification clients). This makes the domain pure and easy to reason about.
- **Testability**: With the domain logic decoupled from I/O, it can be tested with simple unit tests. Dependencies are injected via "ports" (defined as Rust traits), which allows for easy mocking using libraries like `mockall`.
- **Interchangeability**: Adapters for external services can be swapped out with minimal impact on the core application. For example, the notification target could be switched from Discord to Slack by simply implementing a new adapter.
- **Clear Dependency Rule**: Dependencies always point inward towards the domain, preventing the business logic from getting entangled with implementation details.

### Layers
- **`domain`**: Contains the core business logic, entities (e.g., `Repository`, `Summary`), and ports (traits) that define contracts for external services. It has no dependencies on other layers.
- **`application`**: Orchestrates the business logic. It contains use cases (e.g., "fetch and summarize trending repos") that are triggered by the presentation layer. It depends only on the `domain` layer.
- **`infrastructure`**: Implements the adapters for the ports defined in the `domain` layer. This is where all external communication happens (e.g., HTTP requests to GitHub, calls to the Gemini API, sending messages to Discord).
- **`presentation`**: The entry point of the application. It's responsible for triggering the application's use cases. This could be a command-line interface, a scheduled job runner, or a web server.

## 3. Setup Instructions

### Prerequisites
- Rust (latest stable version recommended)
- Git

### Installation
1.  **Clone the repository:**
    ```bash
    git clone <repository-url>
    cd github-trending-summary
    ```

2.  **Set up environment variables:**
    Copy the example environment file and fill in the required values.
    ```bash
    cp .env.example .env
    ```
    You will need to provide:
    - `DISCORD_WEBHOOK_URL`: The webhook URL for your Discord channel.
    - `GEMINI_API_KEY`: Your API key for the Google Gemini service.

3.  **Build the application:**
    ```bash
    cargo build --release
    ```

4.  **Run the application:**
    ```bash
    ./target/release/github-trending-summary
    ```

## 4. Development Guidelines

### Coding Style
- Follow the official [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/).
- Run `cargo fmt` before committing to ensure consistent code formatting.
- Run `cargo clippy` to catch common mistakes and improve code quality.

### Testing
- **Unit Tests**: Place unit tests within the module they are testing. The domain logic should be thoroughly covered by unit tests.
- **Integration Tests**: Place integration tests in the `tests/` directory. These tests should cover the interaction between different components, especially the application and infrastructure layers. Use mocks for external services where appropriate.
- All new features should be accompanied by relevant tests.

## 5. Roadmap

- [ ] Implement GitHub scraper in the `infrastructure` layer.
- [ ] Implement Gemini AI summarizer in the `infrastructure` layer.
- [ ] Implement Discord notifier in the `infrastructure` layer.
- [ ] Implement the main use case in the `application` layer.
- [ ] Add robust error handling and logging throughout the application.
- [ ] Set up a scheduler in the `presentation` layer to run the process daily.
- [ ] Add support for more summarization models.
- [ ] Add more configuration options (e.g., summary format, notification frequency).
