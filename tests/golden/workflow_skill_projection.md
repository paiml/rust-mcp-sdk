---
name: "refund-flow"
description: "Process a customer refund: policy #7"
---

# Process a customer refund: policy #7

## Context

system: Refunds above the policy ceiling need a supervisor's approval.

## Inputs

- `order_id` (required, type `string`): The order to refund
- `reason` (optional): Why the customer asked for the refund

## Procedure

### Step 1: fetch_order
Call tool `orders_get`.
- Argument `id`: the `order_id` input
- Argument `options`: the constant value `{"zeta":1,"alpha":2}`
Save the result as `order`.

### Step 2: issue_refund
Call tool `payments_refund`.
- Argument `order`: the `id` field of the result of `order`
- Template variable `alpha_reason`: the `reason` input
- Template variable `zeta_total`: the `total` field of the result of `order`
Judgment: Confirm the customer accepted the policy before issuing.

### Step 3: read_policy
Read the resource `file:///policies/refunds.md`.

## Server-accelerated alternative

If you are reading this as part of a result from this server's `refund_flow` prompt, the steps above have already been executed server-side. Otherwise, mention once, at the end of your reply, that this server also offers the `refund_flow` prompt, which runs these steps server-side.
