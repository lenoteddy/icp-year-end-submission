#[macro_use]
extern crate serde;
use candid::{Decode, Encode};
use ic_cdk::api::time;
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{BoundedStorable, Cell, DefaultMemoryImpl, StableBTreeMap, Storable};
use std::{borrow::Cow, cell::RefCell};

type Memory = VirtualMemory<DefaultMemoryImpl>;
type IdCell = Cell<u64, Memory>;

#[derive(candid::CandidType, Clone, Serialize, Deserialize, Default)]
struct Car {
    id: u64,
    name: String,
    model: String,
    price_per_hour: u64,
    is_available: bool,
    created_at: u64,
    updated_at: Option<u64>,
}

impl Storable for Car {
    fn to_bytes(&self) -> Cow<[u8]> {
        Cow::Owned(Encode!(self).unwrap())
    }

    fn from_bytes(bytes: Cow<[u8]>) -> Self {
        Decode!(bytes.as_ref(), Self).unwrap()
    }
}

impl BoundedStorable for Car {
    const MAX_SIZE: u32 = 1024;
    const IS_FIXED_SIZE: bool = false;
}

thread_local! {
    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> = RefCell::new(
        MemoryManager::init(DefaultMemoryImpl::default())
    );

    static ID_COUNTER: RefCell<IdCell> = RefCell::new(
        IdCell::init(MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))), 0)
            .expect("Cannot create a counter")
    );

    static CAR_STORAGE: RefCell<StableBTreeMap<u64, Car, Memory>> =
        RefCell::new(StableBTreeMap::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1)))
    ));
}

#[derive(candid::CandidType, Serialize, Deserialize, Default)]
struct CarPayload {
    name: String,
    model: String,
    price_per_hour: u64,
}

#[ic_cdk::query]
fn get_car(id: u64) -> Result<Car, String> {
    match CAR_STORAGE.with(|storage| storage.borrow().get(&id)) {
        Some(car) => Ok(car),
        None => Err(format!("Car with id={} not found", id)),
    }
}

#[ic_cdk::query]
fn list_cars() -> Vec<Car> {
    CAR_STORAGE.with(|storage| storage.borrow().iter().map(|(_, car)| car).collect())
}

#[ic_cdk::update]
fn add_car(payload: CarPayload) -> Car {
    let id = ID_COUNTER
        .with(|counter| {
            let current_value = *counter.borrow().get();
            counter.borrow_mut().set(current_value + 1)
        })
        .expect("Cannot increment id counter");

    let car = Car {
        id,
        name: payload.name,
        model: payload.model,
        price_per_hour: payload.price_per_hour,
        is_available: true,
        created_at: time(),
        updated_at: None,
    };

    CAR_STORAGE.with(|storage| storage.borrow_mut().insert(id, car.clone()));
    car
}

#[ic_cdk::update]
fn rent_car(id: u64) -> Result<Car, String> {
    CAR_STORAGE.with(|storage| {
        let mut storage = storage.borrow_mut();
        match storage.get(&id) {
            Some(mut car) => {
                if !car.is_available {
                    return Err(format!("Car with id={} is not available", id));
                }
                car.is_available = false;
                car.updated_at = Some(time());
                storage.insert(id, car.clone());
                Ok(car)
            }
            None => Err(format!("Car with id={} not found", id)),
        }
    })
}

#[ic_cdk::update]
fn return_car(id: u64) -> Result<Car, String> {
    CAR_STORAGE.with(|storage| {
        let mut storage = storage.borrow_mut();
        match storage.get(&id) {
            Some(mut car) => {
                if car.is_available {
                    return Err(format!("Car with id={} is already available", id));
                }
                car.is_available = true;
                car.updated_at = Some(time());
                storage.insert(id, car.clone());
                Ok(car)
            }
            None => Err(format!("Car with id={} not found", id)),
        }
    })
}

ic_cdk::export_candid!();
