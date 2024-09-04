### Report on Concurrent Address Crawling Implementation

In my implementation, I significantly boosted the speed of crawling Bitcoin node addresses by employing a concurrent approach. The key strategy involves initiating multiple tasks simultaneously, ensuring that a high level of concurrency is maintained throughout the process.

**Performance Metrics:**
- **Crawling 5000 addresses:** Typically completed in 2 to 8 seconds.
- **Crawling 15,000 addresses:** Typically completed in 5 to 15 seconds.

**Algorithm Overview:**
- **Concurrency Management:** The algorithm ensures that there are always 20 tasks running concurrently. Each task fetches addresses from different nodes. As soon as one task completes, a new task is immediately started, ensuring that the number of active tasks remains constant.
- **Address Management:** The fetched addresses are directly added to the `crawled_addresses` set. Once the number of crawled addresses exceeds the target (e.g., 5000), all remaining tasks are promptly canceled to conserve resources and speed up the completion time.

**Technical Implementation:**
- **Futures and `select_all`:** I utilized the `futures` crate along with the `select_all` function to efficiently manage the futures and determine which task completes first. This allows for immediate action upon task completion.
- **Shared State Management:** Shared variables such as the `crawled_addresses` set were safely handled using `Arc` and `Mutex` from the standard Rust library, ensuring thread safety and preventing data races.

**Results:**
- The implementation successfully enhanced the crawling speed, dramatically reducing the time required to gather a large number of addresses.
- **Potential Improvements:** The current implementation uses a fixed concurrency limit of 20 tasks. Future improvements could involve setting dynamic concurrency limits based on real-time resource usage, further optimizing performance.
