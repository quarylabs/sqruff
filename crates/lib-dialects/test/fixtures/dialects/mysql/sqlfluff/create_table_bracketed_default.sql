create table `tickets` (
    `id` serial primary key,
    `material_number` varchar(255) default null,
    `material_name` varchar(255) default null,
    `date_created` date not null default (current_date),
    `date_closed` date default null
);
