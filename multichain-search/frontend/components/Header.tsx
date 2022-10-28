import { Link, Icon, Text, HStack, Tooltip, Box } from '@chakra-ui/react';
import React from 'react';
import styles from '../styles/header.module.css'
import { HeaderLink } from './HeaderLink';

export const Header = () => {
    return (
        <div className={styles.header}>
        <Link href="/" fontSize={24} fontWeight="bold"> 
          Blockscout Multi Search
        </Link>
        <div className={styles.right}>
          <HeaderLink url="https://blockscout.com" text="Blockscout"/>
          <HeaderLink url="/about" text="About"/>
        </div>
      </div>
    )
}