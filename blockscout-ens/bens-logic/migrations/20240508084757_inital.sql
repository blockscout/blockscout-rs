
CREATE TABLE protocol (
    id SERIAL PRIMARY KEY,
    slug VARCHAR(255) UNIQUE,
    tld VARCHAR(255) NOT NULL,
    title VARCHAR NOT NULL,
    description VARCHAR NOT NULL,
    icon_url VARCHAR NOT NULL
);


CREATE TABLE network (
    network_id VARCHAR(255) PRIMARY KEY,
    title VARCHAR
);

CREATE TABLE network_protocol (
    network_id VARCHAR(255) REFERENCES network(network_id),
    protocol_id INT REFERENCES protocol(id),
    PRIMARY KEY (network_id, protocol_id)
);
