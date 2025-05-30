struct Product {
    name: String,
    price: f64,
}

struct CartItem {
    product: Product,
    quantity: u32,
}

// TODO: Implement these
fn create_product(name: &str, price: f64) -> Product {
    Product {
        name: name.to_string(),
        price: price,
    }
}

fn calculate_item_total(item: &CartItem) -> f64 {
    // Calculate total for a single item
    item.product.price * item.quantity as f64
}

fn calculate_cart_total(items: &[CartItem]) -> f64 {
    items.iter().map(calculate_item_total).sum()
}

fn main() {
    let apple = create_product("Apple", 1.5);
    let banana = create_product("Banana", 0.8);

    let cart = vec![
        CartItem {
            product: apple,
            quantity: 5,
        },
        CartItem {
            product: banana,
            quantity: 3,
        },
    ];

    let total = calculate_cart_total(&cart);
    println!("Cart total: ${:.2}", total);
}
