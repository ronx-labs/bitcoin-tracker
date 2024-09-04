# Bitcoin crawling challenge

## General instructions

As this code is Eiger's intellectual property, we ask that you do not share this exercise or your solution with anyone but us.

If any task takes significantly longer than suggested, please let us know as we may want to adjust the expected effort to better match the expected duration.

## Step 1: Code review

Expected duration: 1h
Time limit: 2h from the start of the challenge

Here is a simple Rust binary project performing a handshake with a Bitcoin node.

Considering step 2 will consist in using this code to write a Bitcoin network crawler, **perform a code review on this project**. List what you like about it and what you don't. Describe how easy to read and how maintainable this code is. Mention where the code isn't the most idiomatic, ergonomic or reliable. Suggest fixes and adjustements (no need to actually fix these, just suggest the fixes). Treat it like a pull request. 

Write your findings in the `step1.md` file, and submit it to us **within the time limit**.

## Step 2: Feature implementation

Expected duration: 2~3h
Time limit: 24h from the start of the challenge

Please focus on this stage only once you have finished the review, as carefully reading the existing code will give you some hints on completing this step.

**Extend this program with a new feature: network crawling. Make it so this program can collect the addresses of 5000 peers.** 

Be aware that retrieving these addresses may take up to a minute and that some nodes may only give you a single peer address on the first try and a second try might be required. You won't need to validate that all these addresses correspond to responsive nodes, but you'll need to crawl some of those in order to get 5000 peers. We especially recommend you don't even try to connect to ipv6 and Tor addresses.

For the sake of facilitating our review and saving you time and effort, please refrain from fixing all the rough edges you noticed during the review unless you really feel this is necessary, and focus on the crawling feature.

We encourage you to take advantage of the fact this directory is a git repository. You can stay on the main branch.

## Optional step 3: 

Expected duration: 15~30min
Time limit: 24h from the start of the challenge

_Make it work. Make it right. **Make it fast**_

Without writing actual code (since that might take more than 2~3h, and we are not seeking the perfect solution in step 2), please describe in a few sentences how you would improve and scale your solution to collect the most nodes in the shortest time. Mention the methods and tools you would use.

Put your ideas in `step3.md` or leave comments in your code from step 2. Submit it to us **within the time limit**.
