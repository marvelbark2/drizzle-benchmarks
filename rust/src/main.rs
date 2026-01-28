use axum::{
    Json, Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use parking_lot::Mutex;
use rust::{DbPool, establish_connection_pool, models::*, queries::*};
use serde::Deserialize;
use socket2::{Domain, Socket, Type};
use std::{net::SocketAddr, sync::Arc, time::Duration};
use sysinfo::System;
use tokio::net::TcpListener;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

struct AppState {
    pool: DbPool,
    sys: Mutex<System>,
    cpu_warmed_up: Mutex<bool>,
}

#[derive(Deserialize)]
struct LimitOffset {
    limit: Option<i64>,
    offset: Option<i64>,
}

#[derive(Deserialize)]
struct IdParam {
    id: i32,
}

#[derive(Deserialize)]
struct SearchParam {
    term: String,
}

async fn stats_handler(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let needs_warmup = {
        let mut warmed = state.cpu_warmed_up.lock();
        if !*warmed {
            *warmed = true;
            true
        } else {
            false
        }
    };

    if needs_warmup {
        {
            let mut sys = state.sys.lock();
            sys.refresh_cpu_all();
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    let mut sys = state.sys.lock();
    sys.refresh_cpu_all();

    let result: Vec<i32> = sys
        .cpus()
        .iter()
        .map(|cpu| cpu.cpu_usage().round() as i32)
        .collect();

    Json(result)
}

async fn get_customers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LimitOffset>,
) -> Result<Json<Vec<Customer>>, StatusCode> {
    let mut conn = state
        .pool
        .get()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);
    p1(&mut conn, limit, offset)
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_customer_by_id(
    State(state): State<Arc<AppState>>,
    Query(params): Query<IdParam>,
) -> Result<Json<Option<Customer>>, StatusCode> {
    let pool = state.pool.clone();
    let id = params.id;

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p2(&mut conn, id)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn search_customer(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParam>,
) -> Result<Json<Vec<CustomerSearchResult>>, StatusCode> {
    let pool = state.pool.clone();
    let term = params.term;

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p3(&mut conn, &term)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn get_employees(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LimitOffset>,
) -> Result<Json<Vec<Employee>>, StatusCode> {
    let pool = state.pool.clone();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p4(&mut conn, limit, offset)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn get_employee_with_recipient(
    State(state): State<Arc<AppState>>,
    Query(params): Query<IdParam>,
) -> Result<Json<Vec<EmployeeWithRecipient>>, StatusCode> {
    let pool = state.pool.clone();
    let id = params.id;

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p5(&mut conn, id).map(Json).map_err(|e| {
            eprintln!("Error in p5: {:?}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn get_suppliers(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LimitOffset>,
) -> Result<Json<Vec<Supplier>>, StatusCode> {
    let pool = state.pool.clone();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p6(&mut conn, limit, offset)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn get_supplier_by_id(
    State(state): State<Arc<AppState>>,
    Query(params): Query<IdParam>,
) -> Result<Json<Option<Supplier>>, StatusCode> {
    let pool = state.pool.clone();
    let id = params.id;

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p7(&mut conn, id)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn get_products(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LimitOffset>,
) -> Result<Json<Vec<Product>>, StatusCode> {
    let pool = state.pool.clone();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p8(&mut conn, limit, offset)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn get_product_with_supplier(
    State(state): State<Arc<AppState>>,
    Query(params): Query<IdParam>,
) -> Result<Json<Vec<ProductWithSupplier>>, StatusCode> {
    let pool = state.pool.clone();
    let id = params.id;

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p9(&mut conn, id)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn search_product(
    State(state): State<Arc<AppState>>,
    Query(params): Query<SearchParam>,
) -> Result<Json<Vec<ProductSearchResult>>, StatusCode> {
    let pool = state.pool.clone();
    let term = params.term;

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p10(&mut conn, &term)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn get_orders_with_details(
    State(state): State<Arc<AppState>>,
    Query(params): Query<LimitOffset>,
) -> Result<Json<Vec<P11Row>>, StatusCode> {
    let pool = state.pool.clone();
    let limit = params.limit.unwrap_or(100);
    let offset = params.offset.unwrap_or(0);

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p11(&mut conn, limit, offset)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn get_order_with_details(
    State(state): State<Arc<AppState>>,
    Query(params): Query<IdParam>,
) -> Result<Json<Vec<P11Row>>, StatusCode> {
    let pool = state.pool.clone();
    let id = params.id;

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p12(&mut conn, id)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

async fn get_order_with_details_and_products(
    State(state): State<Arc<AppState>>,
    Query(params): Query<IdParam>,
) -> Result<Json<Vec<OrderWithDetailsAndProducts>>, StatusCode> {
    let pool = state.pool.clone();
    let id = params.id;

    tokio::task::spawn_blocking(move || {
        let mut conn = pool.get().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        p13(&mut conn, id)
            .map(Json)
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
}

#[tokio::main]
async fn main() {
    let state = Arc::new(AppState {
        pool: establish_connection_pool(),
        sys: Mutex::new(System::new_all()),
        cpu_warmed_up: Mutex::new(false),
    });

    let app = Router::new()
        .route("/stats", get(stats_handler))
        .route("/customers", get(get_customers))
        .route("/customer-by-id", get(get_customer_by_id))
        .route("/search-customer", get(search_customer))
        .route("/employees", get(get_employees))
        .route("/employee-with-recipient", get(get_employee_with_recipient))
        .route("/suppliers", get(get_suppliers))
        .route("/supplier-by-id", get(get_supplier_by_id))
        .route("/products", get(get_products))
        .route("/product-with-supplier", get(get_product_with_supplier))
        .route("/search-product", get(search_product))
        .route("/orders-with-details", get(get_orders_with_details))
        .route("/order-with-details", get(get_order_with_details))
        .route(
            "/order-with-details-and-products",
            get(get_order_with_details_and_products),
        )
        .with_state(state);

    // Create socket with optimizations for better performance
    let addr: SocketAddr = "0.0.0.0:3003".parse().unwrap();
    let socket = Socket::new(Domain::IPV4, Type::STREAM, None).unwrap();
    socket.set_reuse_address(true).unwrap();
    socket.set_nodelay(true).unwrap();
    socket.bind(&addr.into()).unwrap();
    socket.listen(8192).unwrap();
    socket.set_nonblocking(true).unwrap();
    
    let listener = TcpListener::from_std(socket.into()).unwrap();

    println!("Server running on http://0.0.0.0:3003");

    axum::serve(listener, app)
        .tcp_nodelay(true)
        .await
        .unwrap();
}
