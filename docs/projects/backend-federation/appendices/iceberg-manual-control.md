# Manual Iceberg Control: The Practical Approach

## Yes, You Can Manually Control Iceberg!

Iceberg provides several ways to manually control data management, compaction, and tiering. Here are the practical approaches:

## 1. Manual Compaction Control

### **Trigger Compaction Manually**
```rust
pub struct IcebergManualControl {
    table: Table,
}

impl IcebergManualControl {
    // Manually trigger compaction when you want
    pub async fn compact_table(&self) -> Result<()> {
        // Compact small files into larger ones
        let compaction = Compaction::new()
            .strategy(CompactionStrategy::BinPack)
            .target_file_size(128 * 1024 * 1024) // 128MB target
            .build();
            
        self.table.rewrite_files(compaction).await?;
        Ok(())
    }
    
    // Compact specific partitions
    pub async fn compact_partition(&self, partition: &str) -> Result<()> {
        let partition_filter = PartitionFilter::new()
            .field("date_partition")
            .eq(partition);
            
        let compaction = Compaction::new()
            .filter(partition_filter)
            .strategy(CompactionStrategy::BinPack)
            .build();
            
        self.table.rewrite_files(compaction).await?;
        Ok(())
    }
}
```

## 2. Manual File Management

### **Control File Sizes and Counts**
```rust
impl IcebergManualControl {
    // Manually control file organization
    pub async fn optimize_file_layout(&self) -> Result<()> {
        // Get current file statistics
        let file_stats = self.table.scan_files().await?;
        
        // If too many small files, compact them
        if file_stats.small_file_count > 100 {
            self.compact_table().await?;
        }
        
        // If files too large, split them
        if file_stats.large_file_count > 10 {
            self.split_large_files().await?;
        }
        
        Ok(())
    }
    
    // Manually split large files
    pub async fn split_large_files(&self, max_size: usize) -> Result<()> {
        let large_files = self.table.scan_files()
            .await?
            .filter(|file| file.size_bytes > max_size);
            
        for file in large_files {
            // Split large file into smaller chunks
            self.table.rewrite_files(
                Compaction::new()
                    .filter(PartitionFilter::file_id(file.id))
                    .strategy(CompactionStrategy::Sort)
                    .target_file_size(max_size / 2)
                    .build()
            ).await?;
        }
        Ok(())
    }
}
```

## 3. Manual Partition Management

### **Control Data Partitioning**
```rust
impl IcebergManualControl {
    // Manually partition data by time
    pub async fn partition_by_time(&self, time_field: &str) -> Result<()> {
        let partition_spec = PartitionSpec::builder()
            .with_field(PartitionField::new(
                time_field,
                PartitionTransform::Day
            ))
            .build();
            
        self.table.update_spec(partition_spec).await?;
        Ok(())
    }
    
    // Manually partition by series_id
    pub async fn partition_by_series(&self) -> Result<()> {
        let partition_spec = PartitionSpec::builder()
            .with_field(PartitionField::new(
                "series_id",
                PartitionTransform::Identity
            ))
            .build();
            
        self.table.update_spec(partition_spec).await?;
        Ok(())
    }
}
```

## 4. Manual Data Lifecycle Management

### **Control Data Retention and Archival**
```rust
impl IcebergManualControl {
    // Manually archive old data
    pub async fn archive_old_data(&self, cutoff_date: Date) -> Result<()> {
        let old_data_filter = PartitionFilter::new()
            .field("date")
            .lt(cutoff_date);
            
        // Move old data to archive storage
        self.table.rewrite_files(
            Compaction::new()
                .filter(old_data_filter)
                .strategy(CompactionStrategy::Archive)
                .target_storage_class("ARCHIVE")
                .build()
        ).await?;
        
        Ok(())
    }
    
    // Manually delete old data
    pub async fn delete_old_data(&self, cutoff_date: Date) -> Result<()> {
        let old_data_filter = PartitionFilter::new()
            .field("date")
            .lt(cutoff_date);
            
        // Delete old data completely
        self.table.delete_files(old_data_filter).await?;
        Ok(())
    }
}
```

## 5. Manual Storage Tiering

### **Control Storage Classes**
```rust
impl IcebergManualControl {
    // Manually move data between storage tiers
    pub async fn move_to_hot_storage(&self, series_id: &str) -> Result<()> {
        let filter = PartitionFilter::new()
            .field("series_id")
            .eq(series_id);
            
        self.table.rewrite_files(
            Compaction::new()
                .filter(filter)
                .target_storage_class("HOT")
                .build()
        ).await?;
        Ok(())
    }
    
    // Manually move to cold storage
    pub async fn move_to_cold_storage(&self, series_id: &str) -> Result<()> {
        let filter = PartitionFilter::new()
            .field("series_id")
            .eq(series_id);
            
        self.table.rewrite_files(
            Compaction::new()
                .filter(filter)
                .target_storage_class("COLD")
                .build()
        ).await?;
        Ok(())
    }
}
```

## 6. Manual Schema Evolution

### **Control Schema Changes**
```rust
impl IcebergManualControl {
    // Manually evolve schema
    pub async fn add_column(&self, name: &str, data_type: DataType) -> Result<()> {
        let new_schema = self.table.schema()
            .add_field(Field::new(name, data_type, true))
            .build();
            
        self.table.update_schema(new_schema).await?;
        Ok(())
    }
    
    // Manually rename column
    pub async fn rename_column(&self, old_name: &str, new_name: &str) -> Result<()> {
        self.table.rename_column(old_name, new_name).await?;
        Ok(())
    }
}
```

## 7. Practical Hybrid Approach

### **Combine Automatic + Manual Control**
```rust
pub struct HybridIcebergManager {
    table: Table,
    auto_compaction: bool,
    manual_triggers: ManualTriggers,
}

impl HybridIcebergManager {
    pub async fn smart_management(&self) -> Result<()> {
        // Automatic: Let Iceberg handle basic compaction
        if self.auto_compaction {
            self.table.auto_compact().await?;
        }
        
        // Manual: Control specific optimizations
        self.manual_triggers.execute().await?;
        
        Ok(())
    }
}

pub struct ManualTriggers {
    // Manual triggers for specific scenarios
    pub compact_when_small_files: usize,  // e.g., > 100 small files
    pub compact_when_large_files: usize,   // e.g., > 10 large files
    pub archive_after_days: i32,          // e.g., archive after 365 days
    pub delete_after_days: i32,           // e.g., delete after 5 years
}

impl ManualTriggers {
    pub async fn execute(&self) -> Result<()> {
        // Check conditions and trigger manual operations
        if self.should_compact() {
            self.compact_table().await?;
        }
        
        if self.should_archive() {
            self.archive_old_data().await?;
        }
        
        if self.should_delete() {
            self.delete_old_data().await?;
        }
        
        Ok(())
    }
}
```

## Benefits of Manual Control

### **Why Manual + Iceberg Works Well**
1. **Iceberg Handles Complexity**: Schema evolution, ACID transactions, metadata
2. **You Control Performance**: Manual compaction, partitioning, tiering
3. **Best of Both Worlds**: Automatic data management + manual optimization
4. **Predictable**: You know exactly when operations happen
5. **Debuggable**: Easy to understand what's happening

### **Practical Implementation**
```rust
// Daily maintenance routine
pub async fn daily_maintenance(&self) -> Result<()> {
    // 1. Let Iceberg handle basic compaction automatically
    self.table.auto_compact().await?;
    
    // 2. Manual optimization for specific scenarios
    if self.has_too_many_small_files() {
        self.compact_table().await?;
    }
    
    // 3. Manual archival of old data
    if self.has_old_data_to_archive() {
        self.archive_old_data().await?;
    }
    
    Ok(())
}
```

This gives you **Iceberg's automatic data management** with **manual control over performance optimization** - the best of both worlds!
