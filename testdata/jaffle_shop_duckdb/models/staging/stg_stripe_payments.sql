select * from {{ source('stripe', 'payments') }}
