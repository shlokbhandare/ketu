# Raft Consensus Research - Ketu Phase 4

## 1. The States of Power
- **Follower:** The starting state. They are passive and just listen to the Leader.
- **Candidate:** When a Follower stops hearing "heartbeats" from the Leader, it waits for its random timeout and then promotes itself to a Candidate to start an election.
- **Leader:** The node that won the majority of votes. It sends heartbeats to everyone else to stay in power.

## 2. Randomized Election Timeouts
- **The Problem:** If all nodes realize a leader is dead at the same time, they all become Candidates at once. They vote for themselves, no one gets a majority, and the election loops endlessly (Split Vote).
- **The Solution:** Every node has a random timer (e.g., 150ms to 300ms). The "fastest" node to wake up starts the election first and asks for votes before the others even realize the leader is gone.

## 3. Terms (The Timeline)
- **Concept:** Terms are like election cycles (Term 1, 2, 3...). 
- **Rule of Authority:** A higher Term number always wins. 
- **The Scenario:** If an old Leader (Term 1) wakes up and tries to give orders to a new Leader (Term 2), the new Leader sees the lower number and says "Your time has passed." The old Leader sees the higher number, realizes it’s out of date, and steps down to become a Follower.

## 4. Quorum (N/2 + 1)
- **The Math:** In a 3 node system, you need 2 votes. In 5 nodes, you need 3.
- **The Safety:** By requiring a majority, Raft prevents "Split Brain." You can't have two leaders because it is mathematically impossible for two different nodes to get a majority of votes at the same time. If a node doesn't have the majority's contact/support, it cannot take power.