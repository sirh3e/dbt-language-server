select {{ function("is_positive_int") }}(amount) as ok from raw_payments
