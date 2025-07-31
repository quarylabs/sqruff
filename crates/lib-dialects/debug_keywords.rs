#[cfg(test)]
mod tests {
    use sqruff_lib_dialects::tsql;
    
    #[test]
    fn test_tsql_keywords() {
        let dialect = tsql::dialect();
        let reserved = dialect.sets("reserved_keywords");
        let unreserved = dialect.sets("unreserved_keywords");
        
        println!("Reserved keywords contains IF: {}", reserved.contains("IF"));
        println!("Unreserved keywords contains IF: {}", unreserved.contains("IF"));
        
        let truly_reserved: std::collections::HashSet<_> = reserved
            .difference(&unreserved)
            .collect();
        println!("Truly reserved (reserved - unreserved) contains IF: {}", truly_reserved.contains(&"IF"));
    }
}